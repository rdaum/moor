object MAIL_AGENT
  name: "Mail Distribution Center"
  parent: ROOT_CLASS
  owner: HACKER
  readable: true

  property last_mail_time (owner: HACKER, flags: "r") = 0;
  property max_list_aliases (owner: HACKER, flags: "rc") = 8;
  property max_mail_notify (owner: HACKER, flags: "rc") = 15;
  property options (owner: HACKER, flags: "rc") = {
    "include",
    "noinclude",
    "all",
    "sender",
    "nosubject",
    "expert",
    "enter",
    "sticky",
    "@mail",
    "manymsgs",
    "replyto"
  };
  property "player_default_@mail" (owner: HACKER, flags: "rc") = "last:15";
  property "player_default_@unsend" (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = "last:1";
  property player_expire_time (owner: HACKER, flags: "rc") = 2592000;
  property reserved_patterns (owner: HACKER, flags: "r") = {};
  property time_collisions (owner: HACKER, flags: "r") = {0, 0};

  override aliases = {"Mail Distribution Center", "Postmaster"};
  override description = {
    "This is the database of mailing-list/mail-folder objects.",
    "The basic procedure for creating a new list/folder is to create a child of $mail_recipient (Generic Mail Recipient) assign it a suitable name&aliases, set a suitable .mail_forward/.mail_notify (or create suitable :mail_forward() and :mail_notify() verbs) and then teleport it here.",
    "",
    "Avaliable aliases:",
    ""
  };
  override object_size = {50262, 1084848672};

  verb resolve_addr (this none this) owner: HACKER flags: "rxd"
    "resolve(name,from,seen,prevrcpts,prevnotifs) => {rcpts,notifs} or E_INVARG";
    "resolve(list,from,seen,prevrcpts,prevnotifs) => {bogus,rcpts,notifs}";
    "Given either an address (i.e., objectid) or a list of such, traces down all .mail_forward lists and .mail_notify to determine where a message should actually go and who should be told about it.  Both forms take previous lists of recipients/notifications and add only those addresses that weren't there before.  `seen' is the stack of addresses we are currently resolving (for detecting loops).  The first form returns E_INVARG if `name' is invalid.  The second form returns all invalid addresses in the `bogus' list but still does the appropriate search on the remaining addresses.";
    {recip, from, ?seen = {}, ?prevrcpts = {}, ?prevnotifs = {}} = args;
    sofar = {prevrcpts, prevnotifs};
    if (typeof(recip) == LIST)
      bogus = {};
      for r in (recip)
        result = this:resolve_addr(r, from, seen, @sofar);
        if (result)
          sofar = result;
        else
          bogus = setadd(bogus, r);
        endif
      endfor
      return {bogus, @sofar};
    else
      fwd = include_recip = 0;
      if (recip == $nothing || recip in seen)
        return sofar;
      elseif (!valid(recip) || (!(is_player(recip) || $object_utils:isa(recip, $mail_recipient)) || typeof(fwd = recip:mail_forward(from)) != LIST))
        "recip is a non-player non-mailing-list/folder or forwarding is screwed.";
        if (typeof(fwd) == STR)
          player:tell(fwd);
        endif
        return E_INVARG;
      elseif (is_player(recip) && `recip:refuses_action(from, "mail") ! E_VERBNF')
        player:tell(recip:mail_refused_msg());
        return E_INVARG;
      elseif (fwd)
        if (r = recip in fwd)
          include_recip = 1;
          fwd = listdelete(fwd, r);
        endif
        result = this:resolve_addr(fwd, recip, setadd(seen, recip), @sofar);
        if (bogus = result[1])
          player:tell(recip.name, "(", recip, ")'s .mail_forward list includes the following bogus entr", length(bogus) > 1 ? "ies:  " | "y:  ", $string_utils:english_list(bogus));
        endif
        sofar = result[2..3];
      else
        include_recip = 1;
      endif
      if (ticks_left() < 1000 || seconds_left() < 2)
        suspend(0);
      endif
      biffs = sofar[2];
      for n in (this:mail_notify(recip, from))
        if (valid(n))
          if (i = $list_utils:iassoc_suspended(n, biffs))
            biffs[i] = setadd(biffs[i], recip);
          else
            biffs = {{n, recip}, @biffs};
          endif
        endif
        if (ticks_left() < 1000 || seconds_left() < 2)
          suspend(0);
        endif
      endfor
      return {include_recip ? setadd(sofar[1], recip) | sofar[1], biffs};
    endif
  endverb

  verb sends_to (this none this) owner: HACKER flags: "rxd"
    "sends_to(from,addr,rcpt[,seen]) ==> true iff mail sent to addr passes through rcpt.";
    {from, addr, rcpt, ?seen = {}} = args;
    if (addr == rcpt)
      return 1;
    elseif (!(addr in seen))
      seen = {@seen, addr};
      for a in (typeof(fwd = this:mail_forward(addr, @from ? {} | {from})) == LIST ? fwd | {})
        if (this:sends_to(addr, a, rcpt, seen))
          return 1;
        endif
        $command_utils:suspend_if_needed(0);
      endfor
    endif
    return 0;
  endverb

  verb send_message (this none this) owner: HACKER flags: "rxd"
    "send_message(from,rcpt-list,hdrs,msg) -- formats and sends a mail message.  hders is either the text of the subject line, or a {subject,{reply-to,...}} list.";
    "Return E_PERM if from isn't owned by the caller.";
    "Return {0, @invalid_rcpts} if rcpt-list contains any invalid addresses.  No mail is sent in this case.";
    "Return {1, @actual_rcpts} if successful.";
    {from, to, orig_hdrs, msg} = args;
    "...";
    "... remove bogus Resent-To/Resent-By headers...";
    "...";
    if (typeof(orig_hdrs) == LIST && length(orig_hdrs) > 2)
      hdrs = orig_hdrs[1..2];
      orig_hdrs[1..2] = {};
      strip = {"Resent-To", "Resent-By"};
      for h in (orig_hdrs)
        h[1] in strip || (hdrs = {@hdrs, h});
      endfor
    else
      hdrs = orig_hdrs;
    endif
    "...";
    "... send it...";
    "...";
    if ($perm_utils:controls(caller_perms(), from))
      text = $mail_agent:make_message(from, to, hdrs, msg);
      return this:raw_send(text, to, from);
    else
      return E_PERM;
    endif
  endverb

  verb raw_send (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Copied from Mail Distribution Center (#6418):raw_send by Nosredna (#2487) Mon Feb 24 10:46:26 1997 PST";
    "WIZARDLY";
    "raw_send(text,rcpts,sender) -- does the actual sending of a message.  Assumes that text has already been formatted correctly.  Decides who to send it to and who wants to be notified about it and does so.";
    "Return {E_PERM} if the caller is not entitled to use this verb.";
    "Return {0, @invalid_rcpts} if rcpts contains any invalid addresses.  No mail is sent in this case.";
    "Return {1, @actual_rcpts} if successful.";
    {text, rcpts, from} = args;
    if (typeof(rcpts) != LIST)
      rcpts = {rcpts};
    endif
    if (!(caller in {$mail_agent, $mail_editor}))
      return {E_PERM};
    elseif (bogus = (resolve = this:resolve_addr(rcpts, from))[1])
      return {0, bogus};
    else
      set_task_perms($wiz_utils:random_wizard());
      this:touch(rcpts);
      actual_rcpts = resolve[2];
      biffs = resolve[3];
      results = {};
      for recip in (actual_rcpts)
        if (ticks_left() < 10000 || seconds_left() < 2)
          player:notify(tostr("...", recip));
          suspend(1);
        endif
        if (typeof(e = recip:receive_message(text, from)) in {ERR, STR})
          "...receive_message bombed...";
          player:notify(tostr(recip, ":receive_message:  ", e));
          e = 0;
        elseif (!is_player(recip) || !e)
          "...not a player or receive_message isn't giving out the message number";
          "...no need to force a notification...";
        elseif (i = $list_utils:iassoc(recip, biffs))
          "...player-recipient was already getting a notification...";
          "...make sure notification includes a mention of him/her/itself.";
          if (!(recip in listdelete(biffs[i], 1)))
            (biffs[i])[2..1] = {recip};
          endif
        else
          "...player-recipient wasn't originally being notified at all...";
          biffs = {{recip, recip}, @biffs};
        endif
        results = {@results, e};
      endfor
      "The following is because the scheduler can BITE ME. --Nosredna";
      fork (0)
        for b in (biffs)
          if (ticks_left() < 10000 || seconds_left() < 2)
            suspend(1);
          endif
          if ($object_utils:has_callable_verb(b[1], "notify_mail"))
            mnums = {};
            for r in (listdelete(b, 1))
              mnums = {@mnums, (rn = r in actual_rcpts) && results[rn]};
            endfor
            (b[1]):notify_mail(from, listdelete(b, 1), mnums);
          endif
        endfor
      endfork
      return {1, @actual_rcpts};
    endif
  endverb

  verb "mail_forward mail_notify" (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if ($object_utils:has_verb(who, verb))
      return who:(verb)(@listdelete(args, 1));
    else
      return {};
    endif
  endverb

  verb touch (this none this) owner: HACKER flags: "rxd"
    "touch(name or list,seen) => does .last_used_time = time() if we haven't already touched this in the last hour";
    {recip, ?seen = {}} = args;
    if (typeof(recip) == LIST)
      for r in (recip)
        result = this:touch(r, seen);
        $command_utils:suspend_if_needed(0);
      endfor
    else
      if (!valid(recip) || recip in seen || (!is_player(recip) && !($mail_recipient in $object_utils:ancestors(recip))))
        "recip is neither a player nor a mailing list/folder";
      else
        if (fwd = this:mail_forward(recip))
          this:touch(fwd, {@seen, recip});
        endif
        if (!is_player(recip))
          recip.last_used_time = time();
        endif
      endif
    endif
  endverb

  verb look_self (this none this) owner: HACKER flags: "rxd"
    player:tell_lines(this.description);
    for c in (this.contents)
      $command_utils:suspend_if_needed(0);
      c:look_self();
    endfor
  endverb

  verb acceptable (this none this) owner: HACKER flags: "rxd"
    "Only allow mailing lists/folders in here and only if their names aren't already taken.";
    what = args[1];
    return $object_utils:isa(what, $mail_recipient) && this:check_names(what, @what.aliases) && what:description() != parent(what):description();
  endverb

  verb check_names (this none this) owner: HACKER flags: "rxd"
    "...make sure the list has at least one usable name.";
    "...make sure none of the aliases are already taken.";
    {object, @aliases} = args;
    if (typeof(object) == STR)
      "... legacy; old version of this verb did not take on OBJ argument";
      aliases = args;
    endif
    ok = 0;
    if (length(aliases) > this.max_list_aliases)
      player:tell("Mailing lists may not have more than ", this.max_list_aliases, " aliases.");
      return 0;
    endif
    for a in (aliases)
      sub1 = strsub(a, "_", "-");
      sub2 = strsub(a, "-", "_");
      if (sub1 == sub2)
        check = 0;
      else
        check = 1;
      endif
      if (index(a, " "))
      elseif (rp = $mail_agent:reserved_pattern(a))
        player:tell("Mailing list name \"", a, "\" uses a reserved pattern: ", rp[1]);
        return 0;
      elseif (valid(p = $mail_agent:match(a, #-1)) && (p != object && a in p.aliases))
        player:tell("Mailing list name \"", a, "\" in use on ", p.name, "(", p, ")");
        return 0;
      elseif (check && (valid(p = $mail_agent:match(sub1, #-1)) && (p != object && sub1 in p.aliases)))
        player:tell("Mailing list name \"", sub1, "\" in use on ", p.name, "(", p, ")");
        return 0;
      elseif (check && (valid(p = $mail_agent:match(sub2, #-1)) && (p != object && sub2 in p.aliases)))
        player:tell("Mailing list name \"", sub2, "\" in use on ", p.name, "(", p, ")");
        return 0;
      else
        ok = 1;
      endif
    endfor
    return ok;
  endverb

  verb "match_old match" (this none this) owner: HACKER flags: "rxd"
    ":match(string) => mailing list object in here that matches string.";
    ":match(string,player) => similar but also matches against player's private mailing lists (as kept in .mail_lists).";
    if (!(string = args[1]))
      return $nothing;
    elseif (string[1] == "*")
      string = string[2..$];
    endif
    if (valid(o = $string_utils:literal_object(string)) && $mail_recipient in $object_utils:ancestors(o))
      return o;
    elseif (rp = this:reserved_pattern(string))
      return (rp[2]):match_mail_recipient(string);
    else
      if (valid(who = {@args, player}[2]) && typeof(use = `who.mail_lists ! E_PROPNF, E_PERM') == LIST)
        use = {@this.contents, @use};
      else
        use = this.contents;
      endif
      partial = 1;
      string = strsub(string, "_", "-");
      for l in (use)
        if (string in l.aliases)
          return l;
        endif
        if (partial != $ambiguous_match)
          for a in (l.aliases)
            if (index(a, string) == 1 && !index(a, " "))
              if (partial)
                partial = l;
              elseif (partial != l)
                partial = $ambiguous_match;
              endif
            endif
            $command_utils:suspend_if_needed(0);
          endfor
        endif
      endfor
      return partial && $failed_match;
    endif
  endverb

  verb match_recipient (this none this) owner: HACKER flags: "rxd"
    ":match_recipient(string[,meobj]) => $player or $mail_recipient object that matches string.  Optional second argument (defaults to player) is returned in the case string==\"me\" and is also used to obtain a list of private $mail_recipients to match against.";
    {string, ?me = player} = args;
    if (valid(me) && $failed_match != (o = me:my_match_recipient(string)))
      return o;
    elseif (!string)
      return $nothing;
    elseif (string[1] == "*" && string != "*")
      return this:match(@args);
    elseif (string[1] == "`")
      (args[1])[1..1] = "";
      return $string_utils:match_player(@args);
    elseif (valid(o = $string_utils:match_player(@args)) || o == $ambiguous_match)
      return o;
    else
      return this:match(@args);
    endif
  endverb

  verb match_failed (this none this) owner: HACKER flags: "rxd"
    {match_result, string, ?cmd_id = ""} = args;
    cmd_id = cmd_id || "";
    if (match_result == $nothing)
      player:tell(cmd_id, "You must specify a valid mail recipient.");
    elseif (match_result == $failed_match)
      player:tell(cmd_id, "There is no mail recipient called \"", string, "\".");
    elseif (match_result == $ambiguous_match)
      if ((nostar = index(string, "*") != 1) && (lst = $player_db:find_all(string)))
        player:tell(cmd_id, "\"", string, "\" could refer to ", length(lst) > 20 ? tostr("any of ", length(lst), " players") | $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", lst), "no one", " or "), ".");
      else
        player:tell(cmd_id, "I don't know which \"", nostar ? "*" | "", string, "\" you mean.");
      endif
    elseif (!valid(match_result))
      player:tell(cmd_id, match_result, " does not exist.");
    else
      return 0;
    endif
    return 1;
  endverb

  verb make_message (this none this) owner: HACKER flags: "rxd"
    ":make_message(sender,recipients,subject/replyto/additional-headers,body)";
    " => message in the form as it will get sent.";
    {from, recips, hdrs, body} = args;
    try
      fromowner = from.owner;
    except (E_INVIND)
      raise(E_PERM);
    endtry
    fromline = this:name_list(from);
    if (typeof(recips) != LIST)
      recips = {recips};
    endif
    recips = this:name_list(@recips);
    others = {};
    replyto = 0;
    if (typeof(hdrs) != LIST)
      hdrs = {hdrs};
    endif
    subj = hdrs[1];
    if (!valid(from))
      others = {"Probable-Sender:   " + this:name(fromowner)};
      "  others = {'Possible-Sender:   ' + this:name(player)}";
      "  if (caller_perms() != player)";
      "    others = {@others, 'Possible-Sender:   ' + this:name(caller_perms())}";
      "  endif";
    elseif (!is_player(from))
      others = {"Sender:   " + this:name(from.owner)};
    endif
    replyto = {@hdrs, 0}[2] && this:name_list(@hdrs[2]);
    if (length(hdrs) > 2)
      hdrs[1..2] = {};
      for h in (hdrs)
        if (match(h[1], "[a-z1-9-]+"))
          others = {@others, $string_utils:left(h[1] + ": ", 15) + h[2]};
        endif
      endfor
    endif
    if (typeof(body) != LIST)
      body = body ? {body} | {};
    endif
    return {this:time(), fromline, recips, subj || " ", @replyto ? {"Reply-to: " + replyto} | {}, @others, "", @body};
  endverb

  verb name (this none this) owner: HACKER flags: "rxd"
    what = args[1];
    if (!valid(what))
      name = "???";
    elseif (!is_player(what) && $object_utils:has_callable_verb(what, "mail_name"))
      name = what:mail_name();
    else
      name = what.name;
    endif
    while (m = $code_utils:match_objid(name))
      {s, e} = m[1..2];
      name[s..e] = "";
    endwhile
    return tostr(name, " (", what, ")");
  endverb

  verb name_list (this none this) owner: HACKER flags: "rxd"
    return $string_utils:english_list($list_utils:map_arg(this, "name", args), "no one");
  endverb

  verb parse_address_field (this none this) owner: HACKER flags: "rxd"
    ":parse_address_field(string) => list of objects";
    "This is the standard routine for parsing address lists that appear in From:, To: and Reply-To: lines";
    objects = {};
    string = args[1];
    while (m = $code_utils:match_objid(string))
      {s, e} = m[1..2];
      if (#0 != (o = toobj(string[s + 1..e - 1])))
        objects = {@objects, o};
      endif
      string = string[e + 1..$];
    endwhile
    return objects;
  endverb

  verb display_seq_full (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":display_seq_full(msg_seq[,preamble]) => {cur, last-read-date}";
    "This is the default message display routine.";
    "Prints entire messages on folder (caller) to player.  msg_seq is the handle returned by :parse_message_seq(...) and indicates which messages should be printed.  preamble, if given will precede the output of the message itself, in which case the message number will be substituted for \"%d\".  Returns the number of the final message in the sequence (which can be then used as the new current message number).";
    set_task_perms(caller_perms());
    {msg_seq, ?preamble = ""} = args;
    cur = date = 0;
    for x in (msgs = caller:messages_in_seq(msg_seq))
      cur = x[1];
      date = x[2][1];
      player:display_message(preamble ? strsub(preamble, "%d", tostr(cur)) | {}, caller:msg_full_text(@x[2]));
      if (ticks_left() < 500 || seconds_left() < 2)
        suspend(0);
      endif
    endfor
    return {cur, date};
  endverb

  verb display_seq_headers (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":display_seq_headers(msg_seq[,cur[,last_read_date]])";
    "This is the default header display routine.";
    "Prints a list of headers of messages on caller to player.  msg_seq is the handle returned by caller:parse_message_seq(...).  cur is the player's current message.  last_read_date is the date of the last of the already-read messages.";
    set_task_perms(caller_perms());
    {msg_seq, ?cur = 0, ?last_old = $maxint} = args;
    keep_seq = {@$seq_utils:contract(caller:kept_msg_seq(), $seq_utils:complement(msg_seq, 1, caller:length_all_msgs())), $maxint};
    k = 1;
    mcount = 0;
    width = player:linelen() || 79;
    for x in (msgs = caller:messages_in_seq(msg_seq))
      if (keep_seq[k] <= (mcount = mcount + 1))
        k = k + 1;
      endif
      annot = (d = x[2][1]) > last_old ? "+" | (k % 2 ? " " | "=");
      line = tostr($string_utils:right(x[1], 4, cur == x[1] ? ">" | " "), ":", annot, " ", caller:msg_summary_line(@x[2]));
      player:tell(line[1..min(width, $)]);
      if (ticks_left() < 500 || seconds_left() < 2)
        suspend(0);
      endif
    endfor
    player:tell("----+");
  endverb

  verb rm_message_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":rm_message_seq(msg_seq)  removes the given sequence of from folder (caller)";
    "...removed messages are saved in .messages_going for possible restoration.";
    set_task_perms(caller_perms());
    old = caller.messages;
    new = save = nums = {};
    next = 1;
    for i in [1..length(seq = args[1]) / 2]
      if ($command_utils:running_out_of_time())
        player:tell("... rmm ", old[next][1] - 1);
        suspend(0);
      endif
      start = seq[2 * i - 1];
      new = {@new, @old[next..start - 1]};
      save = {@save, {start - next, old[start..(next = seq[2 * i]) - 1]}};
      nums = {@nums, old[start][1], old[next - 1][1] + 1};
    endfor
    new = {@new, @old[next..$]};
    $command_utils:suspend_if_needed(0, "... rmm ...");
    save_kept = $seq_utils:intersection(caller.messages_kept, seq);
    $command_utils:suspend_if_needed(0, "... rmm ...");
    new_kept = $seq_utils:contract(caller.messages_kept, seq);
    $command_utils:suspend_if_needed(0, "... rmm ...");
    caller.messages_going = save_kept ? {save_kept, save} | save;
    caller.messages = new;
    caller.messages_kept = new_kept;
    if ($object_utils:has_callable_verb(caller, "_fix_last_msg_date"))
      caller:_fix_last_msg_date();
    endif
    return $seq_utils:tostr(nums);
  endverb

  verb undo_rmm (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":undo_rmm()  restores previously deleted messages in .messages_going to .messages.";
    set_task_perms(caller_perms());
    old = caller.messages;
    going = caller.messages_going;
    new = seq = {};
    last = 0;
    next = 1;
    "there are two possible formats here:";
    "OLD: {{n,msgs},{n,msgs},...}";
    "NEW: {kept_seq, {{n,msgs},{n,msgs},...}}";
    if (going && (!(going[1]) || typeof(going[1][2]) == INT))
      kept = going[1];
      going = going[2];
    else
      kept = {};
    endif
    for s in (going)
      new = {@new, @old[last + 1..last + s[1]], @s[2]};
      last = last + s[1];
      seq = {@seq, next + s[1], next = length(new) + 1};
    endfor
    caller.messages = {@new, @old[last + 1..$]};
    caller.messages_going = {};
    caller.messages_kept = $seq_utils:union(kept, $seq_utils:expand(caller.messages_kept, seq));
    if ($object_utils:has_callable_verb(caller, "_fix_last_msg_date"))
      caller:_fix_last_msg_date();
    endif
    return seq;
  endverb

  verb "expunge_rmm list_rmm" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":list_rmm()    displays contents of .messages_going.";
    ":expunge_rmm() destroys contents of .messages_going once and for all.";
    "... both return the number of messages in .messages_going.";
    set_task_perms(caller_perms());
    cmg = caller.messages_going;
    if (cmg && (!(cmg[1]) || typeof(cmg[1][2]) == INT))
      kept = cmg[1];
      cmg = cmg[2];
    else
      kept = {};
    endif
    if (verb == "expunge_rmm")
      caller.messages_going = {};
      count = 0;
      for s in (cmg)
        count = count + length(s[2]);
      endfor
      return count;
    elseif (!cmg)
      return 0;
    else
      msgs = seq = {};
      next = 1;
      for s in (cmg)
        msgs = {@msgs, @s[2]};
        seq = {@seq, next = next + s[1], next = next + length(s[2])};
      endfor
      kept = {@$seq_utils:contract(kept, $seq_utils:complement(seq, 1, $seq_utils:last(seq))), $maxint};
      k = 1;
      mcount = 0;
      for x in (msgs)
        if (kept[k] <= (mcount = mcount + 1))
          k = k + 1;
        endif
        player:tell($string_utils:right(x[1], 4), ":", k % 2 ? "  " | "= ", caller:msg_summary_line(@x[2]));
        if (ticks_left() < 500 || seconds_left() < 2)
          suspend(0);
        endif
      endfor
      if (msgs)
        player:tell("----+");
      endif
      return length(msgs);
    endif
  endverb

  verb renumber (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":renumber([cur]) -- assumes caller is a $mail_recipient or a $player.";
    "...renumbers caller.messages, doing a suspend() if necessary.";
    "...returns {number of messages,new cur}.";
    set_task_perms(caller_perms());
    {?cur = 0} = args;
    caller.messages_going = {};
    "... blow away @rmm'ed messages since there's no way to tell what their new numbers should be...";
    msgs = caller.messages;
    if (cur)
      cur = $list_utils:iassoc_sorted(cur, msgs);
    endif
    while (1)
      "...find first out-of-sequence message...";
      l = 0;
      r = (len = length(msgs)) + 1;
      while (r - 1 > l)
        if (msgs[i = (r + l) / 2][1] > i)
          r = i;
        else
          l = i;
        endif
      endwhile
      "... r == first out-of-sequence, l == last in-sequence, l+1 == r ...";
      if (l >= len)
        return {l, cur};
      endif
      "...renumber as many messages as we have time for...";
      chunk = {};
      while (r <= len && ticks_left() > 3000 && seconds_left() > 2)
        for x in (msgs[r..min(r + 9, len)])
          chunk = {@chunk, {r, x[2]}};
          r = r + 1;
        endfor
      endwhile
      caller.messages = {@msgs[1..l], @chunk, @msgs[r..len]};
      if (chunk)
        player:tell("...(renumbering ", l + 1, " -- ", r - 1, ")");
        suspend(0);
      else
        player:tell("You lose.  This message collection is just too big.");
        return;
      endif
      "... have to be careful since new mail may be received at this point...";
      msgs = caller.messages;
    endwhile
  endverb

  verb msg_summary_line (this none this) owner: HACKER flags: "rxd"
    ":msg_summary_line(@msg) => date/from/subject as a single string.";
    body = ("" in {@args, ""}) + 1;
    if (body > length(args) || !(subject = args[body]))
      subject = "(None.)";
    endif
    if (args[1] < time() - 31536000)
      c = player:ctime(args[1]);
      date = c[5..11] + c[21..25];
    else
      date = player:ctime(args[1])[5..16];
    endif
    from = args[2];
    if (args[4] != " ")
      subject = args[4];
    endif
    return tostr(date, "   ", $string_utils:left(from, 20), "   ", subject);
  endverb

  verb parse_message_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "parse_message_seq(strings,cur[,last_old])";
    "This is the default <message-sequence> parsing routine for those mail commands that refer to sequences of messages (@mail, @read,...) on a folder.";
    "  caller (the folder) is assumed to be a $mail_recipient or a player.";
    "  strings is the <message-sequence> portion of the arg list.";
    "  cur is the number of the player's current message for this folder.";
    "Returns a string error message if the parse fails, otherwise";
    "returns a list {msg_seq, @unused_strings}, where";
    "   msg_seq is a handle understood by caller:display_seq_full/headers(), and ";
    "   unused_strings is the list of remaining uninterpreted strings";
    set_task_perms(caller_perms());
    {strings, ?cur = 0, ?last_old = 0} = args;
    if (!(nummsgs = caller:length_all_msgs()))
      return "%f %<has> no messages.";
    elseif (typeof(strings) != LIST)
      strings = {strings};
    endif
    seq = result = {};
    mode = #0;
    "... changes to 0 if we start seeing message numbers, to 1 if we see masks...";
    keywords = ":from:%from:to:%to:subject:body:before:after:since:until:first:last:kept:unkept";
    keyalist = {{1, "from"}, {6, "%from"}, {12, "to"}, {15, "%to"}, {19, "subject"}, {27, "body"}, {32, "before"}, {39, "after"}, {45, "since"}, {51, "until"}, {57, "first"}, {63, "last"}, {68, "kept"}, {73, "unkept"}};
    strnum = 0;
    for string in (strings)
      strnum = strnum + 1;
      $command_utils:suspend_if_needed(0);
      if (string && ((c = index(string, ":")) && ((k = index(keywords, ":" + string[1..c - 1])) && k == rindex(keywords, ":" + string[1..c - 1]))))
        "...we have a mask to apply...";
        keywd = $list_utils:assoc(k, keyalist)[2];
        if (mode == #0)
          seq = {1, nummsgs + 1};
        endif
        mode = 1;
        if (k <= 27)
          "...from, subject, to, body...";
          pattern = string[c + 1..$];
          if (keywd in {"subject", "body"})
          elseif (keywd[1] == "%")
            pattern = $string_utils:explode(pattern, "|");
          else
            pattern = this:((keywd == "to" ? "_parse_to" | "_parse_from"))(pattern);
            if (typeof(pattern) == STR)
              return pattern;
            endif
          endif
          seq = caller:((keywd + "_msg_seq"))(pattern, seq);
          if (typeof(seq) == STR)
            if (strnum == 1)
              return seq;
            else
              seq = {};
            endif
          endif
        elseif (k <= 51)
          "...before, since, after, until...";
          if (typeof(date = this:_parse_date(string[c + 1..$])) != INT)
            return tostr("Bad date `", string, "':  ", date);
          endif
          s = caller:length_date_le(keywd in {"before", "since"} ? date - 1 | date + 86399);
          if (keywd in {"before", "until"})
            seq = $seq_utils:remove(seq, s + 1, nummsgs);
          else
            seq = $seq_utils:remove(seq, 1, s);
          endif
        elseif (k <= 63)
          "...first, last...";
          if (n = toint(string[c + 1..$]))
            seq = $seq_utils:((keywd + "n"))(seq, n);
          else
            return tostr("Bad number in `", string, "'");
          endif
        else
          "...kept, unkept...";
          if (c < length(string))
            return tostr("Unexpected junk after `", keywd, ":'");
          elseif (!(seq = caller:((keywd + "_msg_seq"))(seq)) && strnum == 1)
            return tostr("%f %<has> no ", keywd, " messages.");
          endif
        endif
      else
        "...continue building the present sequence...";
        if (mode)
          seq && (result = $seq_utils:union(result, seq));
          seq = {};
        endif
        mode = 0;
        if (!string)
          "...default case for @read: get the current message but skip to the next one if it's not there...";
          if (cur)
            i = min(caller:length_num_le(cur - 1) + 1, nummsgs);
            seq = $seq_utils:add(seq, i, i);
          else
            return "%f %<has> no current message.";
          endif
        elseif (index(string, "next") == 1 && !index(string, "-"))
          string[1..4] = "";
          if ((n = string ? toint(string) | 1) <= 0)
            return tostr("Bad number `", string, "'");
          elseif ((i = caller:length_num_le(cur) + 1) <= nummsgs)
            seq = $seq_utils:add(seq, i, min(i + n - 1, nummsgs));
          else
            return "%f %<has> no next message.";
          endif
        elseif (index(string, "prev") == 1 && !index(string, "-"))
          string[1..4] = "";
          if ((n = string ? toint(string) | 1) <= 0)
            return tostr("Bad number `", string, "'");
          elseif (i = caller:length_num_le(cur - 1))
            seq = $seq_utils:add(seq, max(1, i - n + 1), i);
          else
            return "%f %<has> no previous message.";
          endif
        elseif (string == "new")
          s = last_old ? caller:length_date_le(last_old) | caller:length_num_le(cur);
          if (s < nummsgs)
            seq = $seq_utils:add(seq, s + 1, nummsgs);
          else
            return "%f %<has> no new messages.";
          endif
        elseif (string == "first")
          seq = $seq_utils:add(seq, 1, 1);
        elseif (n = toint(string) || (string in {"last", "$"} && -1 || (string == "cur" && cur)))
          if (n <= 0)
            seq = $seq_utils:add(seq, max(0, nummsgs + n) + 1, nummsgs);
          elseif (i = caller:exists_num_eq(n))
            seq = $seq_utils:add(seq, i, i);
          else
            return string == "cur" ? "%f's current message has been removed." | tostr("%f %<has> no message numbered `", string, "'.");
          endif
        elseif ((i = index(string, "..")) > 1 || (i = index(string, "-")) > 1)
          if ((start = toint(sst = string[1..i - 1])) > 0)
            s = caller:length_num_le(start - 1);
          elseif (sst in {"next", "prev", "cur"})
            s = max(0, caller:length_num_le(cur - (sst != "next")) - (sst == "prev"));
          elseif (sst in {"last", "$"})
            s = nummsgs - 1;
          elseif (sst == "first")
            s = 0;
          else
            return {$seq_utils:union(result, seq), @strings[strnum..$]};
          endif
          j = string[i] == "." ? i + 2 | i + 1;
          if ((end = toint(est = string[j..$])) > 0)
            e = caller:length_num_le(end);
          elseif (est in {"next", "prev", "cur"})
            e = min(nummsgs, caller:length_num_le(cur - (est == "prev")) + (est == "next"));
          elseif (est in {"last", "$"})
            e = nummsgs;
          elseif (est == "first")
            e = 1;
          else
            return {$seq_utils:union(result, seq), @strings[strnum..$]};
          endif
          if (s < e)
            seq = $seq_utils:add(seq, s + 1, e);
          else
            return tostr("%f %<has> no messages in range ", string, ".");
          endif
        elseif (string == "cur")
          return "%f %<has> no current message.";
        else
          return {$seq_utils:union(result, seq), @strings[strnum..$]};
        endif
      endif
    endfor
    return {$seq_utils:union(result, seq)};
  endverb

  verb "_parse_from _parse_to" (this none this) owner: HACKER flags: "rxd"
    ":_parse_from(string with |'s in it) => object list";
    ":_parse_to(string with |'s in it) => object list";
    "  for from:string and to:string items in :parse_message_seq";
    if (verb == "_parse_to")
      match_obj = fail_obj = this;
      match_verb = "match_recipient";
      fail_verb = "match_failed";
    else
      match_obj = $string_utils;
      match_verb = "match_player";
      fail_obj = $command_utils;
      fail_verb = "player_match_failed";
    endif
    plist = {};
    for w in ($string_utils:explode(args[1], "|"))
      if (fail_obj:(fail_verb)(p = match_obj:(match_verb)(w), w))
        p = $string_utils:literal_object(w);
        if (p == $failed_match || !$command_utils:yes_or_no("Continue? "))
          return "Bad address list:  " + args[1];
        endif
      endif
      plist = setadd(plist, p);
    endfor
    return plist;
  endverb

  verb _parse_date (this none this) owner: HACKER flags: "rxd"
    words = $string_utils:explode(args[1], "-");
    if (length(words) == 1)
      if (index("yesterday", words[1]) == 1)
        time = $time_utils:dst_midnight(time() - time() % 86400 - 86400);
      elseif (index("today", words[1]) == 1)
        time = $time_utils:dst_midnight(time() - time() % 86400);
      elseif (typeof(time = $time_utils:from_day(words[1], -1)) == ERR)
        time = "weekday, `Today', `Yesterday', or date expected.";
      endif
    elseif (!words || (length(words) > 3 || (!toint(words[1]) || E_TYPE == (year = $code_utils:toint({@words, "-1"}[3])))))
      time = "Date should be of the form `5-Jan', `5-Jan-92', `Wed',`Wednesday'";
    else
      day = toint(words[1]);
      time = $time_utils:dst_midnight($time_utils:from_month(words[2], -1, day));
      if (length(words) == 3)
        thisyear = toint(ctime(time)[21..24]);
        if (100 > year)
          year = thisyear + 50 - (thisyear - year + 50) % 100;
        endif
        time = $time_utils:dst_midnight($time_utils:from_month(words[2], year - thisyear - (year <= thisyear), day));
      endif
    endif
    return time;
  endverb

  verb new_message_num (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":new_message_num() => number that the next incoming message will receive.";
    set_task_perms(caller_perms());
    new = (msgs = caller.messages) ? msgs[$][1] + 1 | 1;
    if (rmsgs = caller.messages_going)
      if (!(rmsgs[1]) || typeof(rmsgs[1][2]) == INT)
        rmsgs = rmsgs[2];
      endif
      lbrm = rmsgs[$][2];
      return max(new, lbrm[$][1] + 1);
    else
      return new;
    endif
  endverb

  verb length_all_msgs (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    return length(caller.messages);
  endverb

  verb length_date_le (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    date = args[1];
    msgs = caller.messages;
    if ((r = length(caller.messages)) < 25)
      for l in [1..r]
        if (msgs[l][2][1] > date)
          return l - 1;
        endif
      endfor
      return r;
    else
      l = 1;
      while (l <= r)
        if (date < msgs[i = (r + l) / 2][2][1])
          r = i - 1;
        else
          l = i + 1;
        endif
      endwhile
      return r;
    endif
  endverb

  verb length_date_gt (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    date = args[1];
    msgs = caller.messages;
    if ((len = length(caller.messages)) < 25)
      for r in [0..len - 1]
        if (msgs[len - r][2][1] <= date)
          return r;
        endif
      endfor
      return len;
    else
      l = 1;
      r = len;
      while (l <= r)
        if (date < msgs[i = (r + l) / 2][2][1])
          r = i - 1;
        else
          l = i + 1;
        endif
      endwhile
      return len - r;
    endif
  endverb

  verb length_num_le (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":length_num_le(num) => number of messages in folder numbered <= num";
    set_task_perms(caller_perms());
    return $list_utils:iassoc_sorted(args[1], caller.messages);
  endverb

  verb exists_num_eq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":exists_num_eq(num) => index of message in folder numbered == num";
    set_task_perms(caller_perms());
    return (i = $list_utils:iassoc_sorted(args[1], caller.messages)) && (caller.messages[i][1] == args[1] && i);
  endverb

  verb from_msg_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":from_msg_seq(object or list[,mask])";
    " => msg_seq of messages from any of these senders";
    set_task_perms(caller_perms());
    {plist, ?mask = {1}} = args;
    if (typeof(plist) != LIST)
      plist = {plist};
    endif
    i = 1;
    fseq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        fromline = msg[2][2];
        for f in ($mail_agent:parse_address_field(fromline))
          if (f in plist)
            fseq = $seq_utils:add(fseq, i, i);
          endif
        endfor
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return fseq || "%f %<has> no messages from " + $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", plist), "no one", " or ");
  endverb

  verb "%from_msg_seq" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":%from_msg_seq(string or list of strings[,mask])";
    " => msg_seq of messages with one of these strings in the from line";
    set_task_perms(caller_perms());
    {nlist, ?mask = {1}} = args;
    if (typeof(nlist) != LIST)
      nlist = {nlist};
    endif
    i = 1;
    fseq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        fromline = " " + msg[2][2];
        for n in (nlist)
          if (index(fromline, n))
            fseq = $seq_utils:add(fseq, i, i);
          endif
        endfor
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return fseq || "%f %<has> no messages from " + $string_utils:english_list($list_utils:map_arg($string_utils, "print", nlist), "no one", " or ");
  endverb

  verb to_msg_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":to_msg_seq(object or list[,mask]) => msg_seq of messages to those people";
    set_task_perms(caller_perms());
    {plist, ?mask = {1}} = args;
    if (typeof(plist) != LIST)
      plist = {plist};
    endif
    i = 1;
    seq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        toline = msg[2][3];
        for r in ($mail_agent:parse_address_field(toline))
          if (r in plist)
            seq = $seq_utils:add(seq, i, i);
          endif
        endfor
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return seq || "%f %<has> no messages to " + $string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%n (%#)", plist), "no one", " or ");
  endverb

  verb "%to_msg_seq" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":%to_msg_seq(string or list of strings[,mask])";
    " => msg_seq of messages containing one of strings in the to line";
    set_task_perms(caller_perms());
    {nlist, ?mask = {1}} = args;
    if (typeof(nlist) != LIST)
      nlist = {nlist};
    endif
    i = 1;
    seq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        toline = " " + msg[2][3];
        for n in (nlist)
          if (index(toline, n))
            seq = $seq_utils:add(seq, i, i);
          endif
        endfor
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return seq || "%f %<has> no messages to " + $string_utils:english_list($list_utils:map_arg($string_utils, "print", nlist), "no one", " or ");
  endverb

  verb subject_msg_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":subject_msg_seq(target) => msg_seq of messages with target in the Subject:";
    set_task_perms(caller_perms());
    {target, ?mask = {1}} = args;
    i = 1;
    seq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif (length(mask) < 2 || i < mask[2])
        subject = msg[2][4];
        if (index(subject, target))
          seq = $seq_utils:add(seq, i, i);
        endif
      else
        mask = mask[3..$];
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return seq || "%f %<has> no messages with subjects containing `" + target + "'";
  endverb

  verb body_msg_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":body_msg_seq(target[,mask]) => msg_seq of messages with target in the body";
    set_task_perms(caller_perms());
    {target, ?mask = {1}} = args;
    i = 1;
    seq = {};
    for msg in (caller.messages)
      if (!mask || i < mask[1])
      elseif ({@mask, $maxint}[2] <= i)
        mask = mask[3..$];
        "Old code follows. Lets save ticks and munge up the whole message body into one big string and index it. Don't need to know where target is in there, just that it is or isn't there";
      elseif ((bstart = "" in (msg = msg[2])) && length(msg) > bstart && index(tostr(@msg[bstart + 1..$]), target))
        seq = $seq_utils:add(seq, i, i);
        "elseif ((bstart = \"\" in (msg = msg[2])) && (l = length(msg)) > bstart)";
        "while (!index(msg[l], target) && (l = l - 1) > bstart)";
        "$command_utils:suspend_if_needed(0);";
        "endwhile";
        "if (l > bstart)";
        "seq = $seq_utils:add(seq, i, i);";
        "endif";
      endif
      i = i + 1;
      $command_utils:suspend_if_needed(0);
    endfor
    return seq || tostr("%f %<has> no messages containing `", target, "' in the body.");
  endverb

  verb messages_in_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":messages_in_seq(msg_seq) => list of messages in msg_seq on folder (caller)";
    set_task_perms(caller_perms());
    if (typeof(msgs = args[1]) != LIST)
      return caller.messages[msgs];
    elseif (length(msgs) == 2)
      return caller.messages[msgs[1]..msgs[2] - 1];
    else
      return $seq_utils:extract(msgs, caller.messages);
    endif
  endverb

  verb __convert_new (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":__convert_new(@msg) => msg in new format (if it isn't already)";
    "               ^ don't forget the @ here.";
    "If the msg is already in the new format it passes through unchanged.";
    "If the msg format is unrecognizable, warnings are printed.";
    if (typeof(date = args[1]) != INT)
      date = 0;
      start = 1;
    else
      start = 2;
      if (!((colon = index(args[2], ":")) && (args[2])[1..colon] in {"From:", "To:", "Subject:"}))
        return args;
      endif
    endif
    from = to = 0;
    subject = " ";
    blank = "" in {@args, ""};
    newhdr = {};
    for line in (args[start..blank - 1])
      if (index(line, "Date:") == 1)
        if (date)
          player:notify("Warning: two dates?");
        endif
        date = $time_utils:from_ctime(line[6..$]);
      elseif (index(line, "From:") == 1)
        if (from)
          player:notify("Warning: two from-lines?");
        endif
        from = $string_utils:triml(line[6..$]);
      elseif (index(line, "To:") == 1)
        if (to)
          player:notify("Warning: two to-lines?");
        endif
        to = $string_utils:triml(line[6..$]);
      elseif (index(line, "Subject:") == 1)
        subject = $string_utils:triml(line[9..$]);
      else
        newhdr = {@newhdr, line};
      endif
    endfor
    if (!from)
      player:notify("Warning: no from-line.");
    endif
    if (!to)
      player:notify("Warning: no to-line.");
    endif
    return {date, from, to, subject, @newhdr, @args[blank..$]};
  endverb

  verb to_text (this none this) owner: HACKER flags: "rxd"
    ":to_text(@msg) => message in text form (suitable for printing)";
    return {"Date:     " + player:ctime(args[1]), "From:     " + args[2], "To:       " + args[3], @args[4] == " " ? {} | {"Subject:  " + args[4]}, @args[5..$]};
  endverb

  verb "is_readable_by is_writable_by is_usable_by" (this none this) owner: HACKER flags: "rxd"
    what = args[1];
    if ($object_utils:isa(what, $mail_recipient))
      return what:(verb)(@listdelete(args, 1));
    else
      "...it's a player:";
      "...  anyone can send mail to it.";
      "...  only the player itself or a wizard can read it.";
      return verb == "is_usable_by" || $perm_utils:controls(args[2], what);
    endif
  endverb

  verb reserved_pattern (this none this) owner: HACKER flags: "rxd"
    ":reserved_pattern(string)";
    "  if string matches one of the reserved patterns for mailing list names, ";
    "  we return that element of .reserved_patterns.";
    string = args[1];
    for p in (this.reserved_patterns)
      if (match(string, p[1]))
        return p;
      endif
    endfor
    return 0;
  endverb

  verb is_recipient (this none this) owner: HACKER flags: "rxd"
    return valid(what = args[1]) && ($mail_recipient_class in (ances = $object_utils:ancestors(what)) || $mail_recipient in ances);
  endverb

  verb keep_message_seq (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":keep_message_seq(msg_seq)";
    "...If msg_seq nonempty {}, this marks the indicated messages on this folder (caller)";
    "...as immune from expiration.";
    "...If msg_seq == {}, this clears all such marks.";
    set_task_perms(caller_perms());
    msg_seq = args[1];
    if (!msg_seq)
      caller.messages_kept = {};
      return 1;
    endif
    prev_kept = caller.messages_kept;
    caller.messages_kept = new_kept = $seq_utils:union(prev_kept, msg_seq);
    added = $seq_utils:intersection(new_kept, $seq_utils:complement(prev_kept));
    if (!added)
      return "";
    endif
    "... urk.  now we need to get the actual numbers of the messages being kept.";
    nums = {};
    start = 0;
    for a in (added)
      nums = {@nums, (start = !start) ? caller:messages_in_seq(a)[1] | caller:messages_in_seq(a - 1)[1] + 1};
    endfor
    return $seq_utils:tostr(nums);
  endverb

  verb "kept_msg_seq unkept_msg_seq" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":kept_msg_seq([mask])";
    " => msg_seq of messages that are marked kept";
    ":unkept_msg_seq([mask])";
    " => msg_seq of messages that are not marked kept";
    set_task_perms(caller_perms());
    {?mask = {1}} = args;
    if (k = verb == "kept_msg_seq")
      kseq = $seq_utils:intersection(mask, caller.messages_kept);
    else
      kseq = $seq_utils:intersection(mask, $seq_utils:range(1, caller:length_all_msgs()), $seq_utils:complement(caller.messages_kept));
    endif
    return kseq;
  endverb

  verb msg_seq_to_msg_num_string (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":msg_seq_to_msg_num_string(msg_seq) => string giving the corresponding message numbers";
    set_task_perms(caller_perms());
    return $seq_utils:tostr($seq_utils:from_list($list_utils:slice(caller:messages_in_seq(args[1]))));
  endverb

  verb msg_seq_to_msg_num_list (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":msg_seq_to_msg_num_list(msg_seq) => list of corresponding message numbers";
    set_task_perms(caller_perms());
    return $list_utils:slice(caller:messages_in_seq(args[1]));
  endverb

  verb send_log_message (this none this) owner: HACKER flags: "rxd"
    "send_log_message(perms,from,rcpt-list,hdrs,msg) -- formats and sends a mail message. hders is either the text of the subject line, or a {subject,{reply-to,...}} list.";
    "KLUDGE.  this may go away.";
    "Send a message while supplying a different permission for use by :mail_forward to determine moderation action.";
    "Return E_PERM unless called by a wizard.";
    "Return {0, @invalid_rcpts} if rcpt-list contains any invalid addresses.  No mail is sent in this case.";
    "Return {1, @actual_rcpts} if successful.";
    {perms, from, to, hdrs, msg} = args;
    if (caller_perms().wizard)
      text = $mail_agent:make_message(from, to, hdrs, msg);
      return this:raw_send(text, to, perms);
    else
      return E_PERM;
    endif
  endverb

  verb parse_misc_headers (this none this) owner: HACKER flags: "rxd"
    ":parse_misc_headers(msg,@extract_names)";
    "Extracts the miscellaneous (i.e., not including Date: From: To: Subject:)";
    "from msg (a mail message in the usual transmission format).";
    "Extract_names is a list of header names";
    "=> {other_headers,bogus_headers,extract_texts,body}";
    "where each element of extract_texts is a string or 0";
    "  according as the corresponding header in extract_names is present.";
    "bogus_headers is a list of those headers that are undecipherable";
    "other_headers is a list of {header_name,header_text} for the remaining";
    "  miscellaneous headers.";
    "headers in msg";
    msgtxt = args[1];
    extract_names = listdelete(args, 1);
    extract_texts = $list_utils:make(length(extract_names));
    heads = bogus = {};
    for h in (msgtxt[5..(bstart = "" in {@msgtxt, ""}) - 1])
      if (m = match(h, "%([a-z1-9-]+%): +%(.*%)"))
        hname = h[m[3][1][1]..m[3][1][2]];
        htext = h[m[3][2][1]..m[3][2][2]];
        if (i = hname in extract_names)
          extract_texts[i] = htext;
        else
          heads = {@heads, {hname, htext}};
        endif
      else
        bogus = {@bogus, h};
      endif
    endfor
    return {heads, bogus, extract_texts, msgtxt[bstart + 1..$]};
  endverb

  verb resend_message (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "resend_message(new_sender,new_rcpts,from,to,hdrs,body)";
    " -- reformats and resends a previously sent message to new recipients.";
    "msg is the previous message";
    "Return E_PERM if new_sender isn't owned by the caller.";
    "Return {0, @invalid_rcpts} if new_rcpts contains any invalid addresses.  No mail is sent in this case.";
    "Return {1, @actual_rcpts} if successful.";
    {new_sender, new_rcpts, from, to, hdrs, body} = args;
    if (typeof(hdrs) != LIST)
      hdrs = {hdrs, 0};
    elseif (length(hdrs) < 2)
      hdrs = {@hdrs || {""}, 0};
    endif
    hdrs[3..2] = {{"Resent-By", this:name_list(new_sender)}, {"Resent-To", this:name_list(@new_rcpts)}};
    if ($perm_utils:controls(caller_perms(), new_sender))
      text = $mail_agent:make_message(from, to, hdrs, body);
      return this:raw_send(text, new_rcpts, new_sender);
    else
      return E_PERM;
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rx"
    if (caller_perms().wizard)
      this.reserved_patterns = {};
      this.last_mail_time = 0;
      this.time_collisions = {0, 0};
      pass(@args);
    endif
  endverb

  verb time (this none this) owner: HACKER flags: "rxd"
    "This was inspired by Xeric's port 4632 on *Core-DB-Issues";
    now = time();
    return now;
    "skipping the below for now because the mail system's clock is getting very out of sync. suspect someone's playing games to run up the clock. HTC 6 Jan 2003";
    if (caller == this)
      if (now > this.last_mail_time)
        return this.last_mail_time = now;
      else
        this.time_collisions[2] = this.time_collisions[2] + 1;
        return this.last_mail_time = this.last_mail_time + 1;
      endif
    else
      return now;
    endif
  endverb

  verb set_message_body_by_index (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":set_message_body_by_index(i,newbody)";
    "Replaces the body of the i-th message on the (caller) recipient.";
    "i must be a message index (not a message number) in the range 1 .. number of messages,";
    "newbody must be a list of strings.";
    set_task_perms(caller_perms());
    {i, body} = args;
    bstart = "" in caller.messages[i][2];
    if (bstart)
      (caller.messages[i][2])[bstart + 1..$] = body;
    else
      (caller.messages[i][2])[$ + 1..$] = {"", @body};
    endif
  endverb

  verb get_message_body_by_index (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
  endverb

  verb message_body_by_index (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":message_body_by_index(i)";
    "Return the body of the i-th message on the (caller) recipient.";
    "i must be a message index (not a message number) in the range 1 .. number of messages,";
    set_task_perms(caller_perms());
    {i} = args;
    msg = caller:messages_in_seq({i, i + 1})[1][2];
    bstart = "" in msg;
    return msg[bstart ? bstart + 1 | $ + 1..$];
  endverb
endobject
