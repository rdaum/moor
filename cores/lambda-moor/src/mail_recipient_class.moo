object MAIL_RECIPIENT_CLASS
  name: "Generic Mail Receiving Player"
  parent: PLAYER
  owner: #2
  fertile: true
  readable: true

  property _mail_task (owner: #2, flags: "rc") = 0;
  property current_message (owner: #2, flags: "c") = {0, 0};
  property mail_forward (owner: #2, flags: "rc") = {};
  property mail_lists (owner: #2, flags: "rc") = {};
  property mail_notify (owner: #2, flags: "r") = {{}, {}};
  property mail_options (owner: #2, flags: "rc") = {};
  property message_keep_date (owner: #2, flags: "rc") = 0;
  property messages (owner: #2, flags: "c") = {};
  property messages_going (owner: #2, flags: "c") = {};
  property messages_kept (owner: #2, flags: "rc") = {};

  override aliases = {"Generic Mail Receiving Player"};
  override help = MAIL_HELP;
  override object_size = {71323, 1084848672};

  verb mail_forward (this none this) owner: #2 flags: "rxd"
    if (typeof(mf = this.(verb)) == STR)
      return $string_utils:pronoun_sub(mf, @args);
    else
      return mf;
    endif
  endverb

  verb receive_message (this none this) owner: #2 flags: "rxd"
    ":receive_message(msg,from)";
    if (!$perm_utils:controls(caller_perms(), this) && caller != this)
      return E_PERM;
    endif
    if (this:mail_option("no_dupcc", args[1][1], args[1][2]))
      "pass to :mail_option the TEXT versions of who the message is from and to";
      recipients = setremove($mail_agent:parse_address_field(args[1][3]), this);
      for x in (recipients)
        if (this:get_current_message(x))
          return 0;
        endif
      endfor
    endif
    if (this:mail_option("netmail"))
      msg = args[1];
      message = {"Forwarded: " + msg[4], "Original-date: " + ctime(msg[1]), "Original-From: " + msg[2], "Original-To: " + msg[3], "Reply-To: " + $string_utils:substitute(args[2].name, {{"@", "%"}}) + "@" + $network.moo_name + ".moo.mud.org"};
      for x in (msg[5..$])
        message = {@message, @$generic_editor:fill_string(x, this:linelen())};
      endfor
      if (this:send_self_netmail(message, @listdelete(args, 1)) == 0)
        return 0;
      endif
    endif
    set_task_perms(this.owner);
    new = this:new_message_num();
    ncur = new <= 1 ? 0 | min(this:current_message(this), new);
    this:set_current_message(this, ncur);
    new = max(new, ncur + 1);
    this.messages = {@this.messages, {new, args[1]}};
    "... new-mail notification is now done directly by $mail_agent:raw_send";
    "... see :notify_mail...";
    return new;
  endverb

  verb display_message (this none this) owner: #2 flags: "rxd"
    ":display_message(preamble,msg) --- prints msg to player.";
    vb = this._mail_task == task_id() || caller == $mail_editor ? "notify_lines_suspended" | "tell_lines_suspended";
    preamble = args[1];
    player:(vb)({@typeof(preamble) == LIST ? preamble | {preamble}, @args[2], "--------------------------"});
  endverb

  verb "parse_message_seq from_msg_seq %from_msg_seq to_msg_seq %to_msg_seq subject_msg_seq body_msg_seq kept_msg_seq unkept_msg_seq display_seq_headers display_seq_full messages_in_seq list_rmm new_message_num length_num_le length_date_le length_date_gt length_all_msgs exists_num_eq msg_seq_to_msg_num_list msg_seq_to_msg_num_string rm_message_seq undo_rmm expunge_rmm renumber keep_message_seq" (this none this) owner: #2 flags: "rxd"
    "parse_message_seq(strings,cur)         => msg_seq";
    "messages_in_seq(msg_seq);              => text of messages in msg_seq";
    "display_seq_headers(msg_seq[,current]) :displays summary lines of those msgs";
    "rmm_message_seq(msg_seq)               => string giving msg numbers removed";
    "undo_rmm()    => msg_seq of restored messages";
    "expunge_rmm() => number of messages expunged";
    "list_rmm()    => number of messages awaiting expunge";
    "renumber(cur) => {number of messages in folder, new_cur}";
    "";
    "See the corresponding routines on $mail_agent.";
    if (caller == $mail_agent || $perm_utils:controls(caller_perms(), this))
      set_task_perms(this.owner);
      return $mail_agent:(verb)(@args);
    else
      return E_PERM;
    endif
  endverb

  verb msg_summary_line (this none this) owner: HACKER flags: "rxd"
    return $mail_agent:msg_summary_line(@args);
  endverb

  verb msg_text (this none this) owner: #2 flags: "rxd"
    ":msg_text(@msg) => list of strings.";
    "msg is a mail message (in the usual transmission format) being read BY this player.";
    "The default version of recipient:msg_full_text calls this to obtain the actual list of strings to display.  (this is a badly named verb).";
    "returns the actual list of strings to display.";
    return $mail_agent:to_text(@args);
  endverb

  verb notify_mail (this none this) owner: #2 flags: "rxd"
    ":notify_mail(from,recipients[,msgnums])";
    " used by $mail_agent:raw_send to notify this player about mail being sent";
    " from <from> to <recipients>.  <msgnums> if given gives the message number(s) assigned (in the event that the corresponding recipient actually kept the mail)";
    if (!$object_utils:connected(this))
      return;
    elseif (!(caller in {this, $mail_agent} || $perm_utils:controls(caller_perms(), this)))
      return E_PERM;
    else
      {from, recipients, ?msgnums = {}} = args;
      from_name = $mail_agent:name(from);
      "... msgnums may be shorter than recipients or may have some slots filled";
      "... with 0's if msg numbers are not available for some recipients.";
      if ((t = this in recipients) && (length(msgnums) >= t && msgnums[t]))
        "... you are getting the mail and moreover your :receive_message kept it.";
        namelist = $string_utils:english_list($list_utils:map_arg($mail_agent, "name", setremove(recipients, this)), "");
        this:notify(tostr("You have new mail (", msgnums[t], ") from ", from_name, namelist ? " which was also sent to " + namelist | "", "."));
        if (!this:mail_option("expert"))
          this:notify(tostr("Type `help mail' for info on reading it."));
        endif
      else
        "... vanilla notification; somebody got sent mail and you're finding out.";
        namelist = $string_utils:english_list({@t ? {"You"} | {}, @$list_utils:map_arg($mail_agent, "name", setremove(recipients, this))}, "");
        this:tell(tostr(namelist, length(recipients) == 1 ? " has" | " have", " just been sent new mail by ", from_name, "."));
      endif
    endif
  endverb

  verb current_message (this none this) owner: #2 flags: "rxd"
    ":current_message([recipient])";
    " => current message number for the given recipient (defaults to this).";
    " => 0 if we have no record of that recipient";
    "      or current message happens to be 0.";
    "This verb is mostly obsolete; consider using :get_current_message()";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      raise(E_PERM);
    elseif (!args || args[1] == this)
      return this.current_message[1];
    elseif (a = $list_utils:assoc(args[1], this.current_message))
      return a[2];
    else
      return 0;
    endif
  endverb

  verb get_current_message (this none this) owner: #2 flags: "rxd"
    ":get_current_message([recipient])";
    " => {msg_num, last_read_date} for the given recipient.";
    " => 0 if we have no record of that recipient.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      raise(E_PERM);
    elseif (!args || args[1] == this)
      if (length(this.current_message) < 2)
        "Whoops, this got trashed---fix it up!";
        this.current_message = {0, time(), @this.current_message};
      endif
      return (this.current_message)[1..2];
    elseif (a = $list_utils:assoc(args[1], this.current_message))
      return a[2..3];
    else
      return 0;
    endif
  endverb

  verb set_current_message (this none this) owner: #2 flags: "rxd"
    ":set_current_message(recipient[,number[,date]])";
    "Returns the new {number,last-read-date} pair for recipient.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      raise(E_PERM);
    endif
    {recip, ?number = E_NONE, ?date = 0, ?force = 0} = args;
    cm = this.current_message;
    if (recip == this)
      this.current_message[2] = max(date, cm[2]);
      if (number != E_NONE)
        this.current_message[1] = number;
      endif
      return (this.current_message)[1..2];
    elseif (i = $list_utils:iassoc(recip, cm))
      if (force)
        "`force' is assumed to come from `@unread'";
        return (this.current_message[i] = {recip, number, date})[2..3];
      else
        return (this.current_message[i] = {recip, number == E_NONE ? cm[i][2] | number, max(date, cm[i][3])})[2..3];
      endif
    else
      entry = {recip, number != E_NONE && number, date};
      this.current_message = {@cm, entry};
      return entry[2..3];
    endif
  endverb

  verb make_current_message (this none this) owner: #2 flags: "rxd"
    ":make_current_message(recipient[,index])";
    "starts a new current_message record for recipient.";
    "index, if given, indicates where recipient is to be";
    "  placed (n = at or after nth entry in .current_message).";
    recip = args[1];
    cm = this.current_message;
    if (length(args) > 1)
      i = max(2, min(args[2], length(cm)));
    else
      i = 0;
    endif
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      raise(E_PERM);
    elseif (recip == this)
      "...self...";
    elseif (j = $list_utils:iassoc(recip, cm))
      "...already present...";
      if (i)
        if (j < i)
          this.current_message = {@cm[1..j - 1], @cm[j + 1..i], cm[j], @cm[i + 1..$]};
        elseif (j > i + 1)
          this.current_message = {@cm[1..i], cm[j], @cm[i + 1..j - 1], @cm[j + 1..$]};
        endif
      endif
    else
      this.current_message = listappend(cm, {recip, 0, 0}, @i ? {i} | {});
    endif
  endverb

  verb kill_current_message (this none this) owner: #2 flags: "rxd"
    ":kill_current_message(recipient)";
    "entirely forgets current message for this recipient...";
    "Returns true iff successful.";
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      raise(E_PERM);
    else
      return (recip = args[1]) != this && ((i = $list_utils:iassoc(recip, cm = this.current_message)) && (this.current_message = listdelete(cm, i)));
    endif
  endverb

  verb current_folder (this none this) owner: #2 flags: "rxd"
    ":current_folder() => default folder to use, always an object, usually `this'";
    set_task_perms(caller_perms());
    return !this:mail_option("sticky") || this.current_folder && this;
  endverb

  verb set_current_folder (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    return this.current_folder = args[1];
  endverb

  verb parse_folder_spec (this none this) owner: #2 flags: "rxd"
    ":parse_folder_spec(verb,args,expected_preposition[,allow_trailing_args_p])";
    " => {folder, msg_seq_args, trailing_args}";
    set_task_perms(caller_perms());
    folder = this:current_folder();
    if (!prepstr)
      return {folder, args[2], {}};
    endif
    {verb, args, prep, ?extra = 0} = args;
    p = prepstr in args;
    if (prepstr != prep)
      "...unexpected preposition...";
      if (extra && !index(prepstr, " "))
        return {folder, args[1..p - 1], args[p..$]};
      else
        player:tell("Usage:  ", verb, " [<message numbers>] [", prep, " <folder/list-name>]");
      endif
    elseif (!(p < length(args) && (fname = args[p + 1])))
      "...preposition but no iobj...";
      player:tell(verb, " ", $string_utils:from_list(args, " "), " WHAT?");
    elseif ($mail_agent:match_failed(folder = $mail_agent:match_recipient(fname, this), fname))
      "...bogus mail folder...";
    else
      return {folder, args[1..p - 1], args[p + 2..$]};
    endif
    return 0;
  endverb

  verb parse_mailread_cmd (this none this) owner: #2 flags: "rxd"
    ":parse_mailread_cmd(verb,args,default,prep[,trailer])";
    "  handles anything of the form  `VERB message_seq [PREP folder ...]'";
    "    default is the default msg-seq to use if none given";
    "    prep is the expected prepstr (assumes prepstr is set), usually `on'";
    "    trailer, if present and true, indicates trailing args are permitted.";
    "  returns {recipient object, message_seq, current_msg,\"...\"} or 0";
    set_task_perms(caller_perms());
    if (!(pfs = this:parse_folder_spec(@listdelete(args, 3))))
      return 0;
    endif
    {verb, args, default, prep, ?extra = 0} = args;
    folder = pfs[1];
    cur = this:get_current_message(folder) || {0};
    if (typeof(pms = folder:parse_message_seq(pfs[2], @cur)) == LIST)
      rest = {@listdelete(pms, 1), @pfs[3]};
      if (!extra && rest)
        "...everything should have been gobbled by :parse_message_seq...";
        player:tell("I don't understand `", rest[1], "'");
        return 0;
      elseif (pms[1])
        "...we have a nonempty message sequence...";
        return {folder, pms[1], cur, rest};
      elseif (used = length(pfs[2]) + 1 - length(pms))
        "...:parse_message_seq used some words, but didn't get anything out of it";
        pms = "%f %<has> no `" + $string_utils:from_list((pfs[2])[1..used], " ") + "' messages.";
      elseif (typeof(pms = folder:parse_message_seq(default, @cur)) == LIST)
        "...:parse_message_seq used nothing, try the default; wow it worked";
        return {folder, pms[1], cur, rest};
      endif
    elseif (typeof(pms) == ERR)
      player:tell($mail_agent:name(folder), " is not readable by you.");
      if (!$object_utils:isa(folder, $mail_recipient))
        player:tell("Use * to indicate a non-player mail recipient.");
      endif
      return 0;
    endif
    if (folder == this)
      subst = {{"%f's", "Your"}, {"%f", "You"}, {"%<has>", "have"}};
    elseif (is_player(folder))
      subst = {{"%f", folder.name}, {"%<has>", $gender_utils:get_conj("has", folder)}};
    else
      subst = {{"%f", $mail_agent:name(folder)}, {"%<has>", "has"}};
    endif
    player:tell($string_utils:substitute(pms, {@subst, {"%%", "%"}}));
    return 0;
  endverb

  verb "@mail" (any any any) owner: #2 flags: "rxd"
    "@mail <msg-sequence>                --- as in help @mail";
    "@mail <msg-sequence> on <recipient> --- shows mail on mailing list or player.";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    if (p = this:parse_mailread_cmd("@mail", args, this:mail_option("@mail") || $mail_agent.("player_default_@mail"), "on"))
      this:set_current_folder(folder = p[1]);
      msg_seq = p[2];
      seq_size = $seq_utils:size(msg_seq);
      if ((lim = player:mail_option("manymsgs")) && (lim <= seq_size && !$command_utils:yes_or_no(tostr("You are about to see ", seq_size, " message headers.  Continue?"))))
        player:notify(tostr("Aborted.  @mailoption manymsgs=", lim));
        return;
      endif
      if (1 != seq_size)
        player:notify(tostr(seq_size, " messages", folder == this ? "" | " on " + $mail_agent:name(folder), ":"));
      endif
      folder:display_seq_headers(msg_seq, @p[3]);
    endif
  endverb

  verb "@read @peek" (any any any) owner: #2 flags: "rxd"
    "@read <msg>...                  -- as in help @read";
    "@read <msg>... on *<recipient>  -- reads messages on recipient.";
    "@peek ...                       -- like @read, but don't set current message";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    if (p = this:parse_mailread_cmd("@read", args, "", "on"))
      this:set_current_folder(folder = p[1]);
      msg_seq = p[2];
      if ((lim = player:mail_option("manymsgs")) && (lim <= (seq_size = $seq_utils:size(msg_seq)) && !$command_utils:yes_or_no(tostr("You are about to see ", seq_size, " messages.  Continue?"))))
        player:notify(tostr("Aborted.  @mailoption manymsgs=", lim));
        return;
      endif
      this._mail_task = task_id();
      if (cur = folder:display_seq_full(msg_seq, tostr("Message %d", folder == this ? "" | " on " + $mail_agent:name(folder), ":")))
        if (verb != "@peek")
          this:set_current_message(folder, @cur);
        endif
      endif
    endif
  endverb

  verb "@next @prev" (any any any) owner: #2 flags: "rd"
    set_task_perms(player.owner);
    if (dobjstr && !(n = toint(dobjstr)))
      player:notify(tostr("Usage:  ", verb, " [<number>] [on <recipient>]"));
    elseif (dobjstr)
      this:("@read")(tostr(verb[2..5], n), @listdelete(args, 1));
    else
      this:("@read")(verb[2..5], @args);
    endif
  endverb

  verb "@rmm*ail" (any any any) owner: #2 flags: "rd"
    "@rmm <message-sequence> [from <recipient>].   Use @unrmm if you screw up.";
    " Beware, though.  @unrmm can only undo the most recent @rmm.";
    set_task_perms(player);
    if (!(p = this:parse_mailread_cmd("@rmm", args, "cur", "from")))
      "...parse failed, we've already complained...";
    elseif (!prepstr && (p[1] != this && !$command_utils:yes_or_no("@rmmail from " + $mail_agent:name(p[1]) + ".  Continue?")))
      "...wasn't the folder player was expecting...";
      player:notify("@rmmail aborted.");
    else
      this:set_current_folder(folder = p[1]);
      e = folder:rm_message_seq(p[2]);
      if (typeof(e) == ERR)
        player:notify(tostr($mail_agent:name(folder), ":  ", e));
      else
        count = (n = $seq_utils:size(p[2])) == 1 ? "." | tostr(" (", n, " messages).");
        fname = folder == this ? "" | " from " + $mail_agent:name(folder);
        player:notify(tostr("Deleted ", e, fname, count));
      endif
    endif
  endverb

  verb "@renumber" (any none none) owner: #2 flags: "rd"
    set_task_perms(player);
    if (!dobjstr)
      folder = this:current_folder();
    elseif ($mail_agent:match_failed(folder = $mail_agent:match_recipient(dobjstr), dobjstr))
      return;
    endif
    cur = this:current_message(folder);
    fname = $mail_agent:name(folder);
    if (typeof(h = folder:renumber(cur)) == ERR)
      player:notify(tostr(h));
    else
      if (!h[1])
        player:notify(tostr("No messages on ", fname, "."));
      else
        player:notify(tostr("Messages on ", fname, " renumbered 1-", h[1], "."));
        this:set_current_folder(folder);
        if (h[2] && this:set_current_message(folder, h[2]))
          player:notify(tostr("Current message is now ", h[2], "."));
        endif
      endif
    endif
  endverb

  verb "@unrmm*ail" (any any any) owner: #2 flags: "rd"
    "@unrmm [on <recipient>]  -- undoes the previous @rmm on that recipient.";
    set_task_perms(player);
    if (!(p = this:parse_folder_spec("@unrmm", args, "on")))
      return;
    endif
    dobjstr = $string_utils:from_list(p[2], " ");
    keep = 0;
    if (!dobjstr || (keep = index("keep", dobjstr) == 1))
      do = "undo_rmm";
    elseif (index("expunge", dobjstr) == 1)
      do = "expunge_rmm";
    elseif (index("list", dobjstr) == 1)
      do = "list_rmm";
    else
      player:notify(tostr("Usage:  ", verb, " [expunge|list] [on <recipient>]"));
      return;
    endif
    this:set_current_folder(folder = p[1]);
    if (msg_seq = folder:(do)(@keep ? {keep} | {}))
      if (do == "undo_rmm")
        player:notify(tostr($seq_utils:size(msg_seq), " messages restored to ", $mail_agent:name(folder), "."));
        folder:display_seq_headers(msg_seq, 0);
      else
        player:notify(tostr(msg_seq, " zombie message", msg_seq == 1 ? " " | "s ", do == "expunge_rmm" ? "expunged from " | "on ", $mail_agent:name(folder), "."));
      endif
    elseif (typeof(msg_seq) == ERR)
      player:notify(tostr($mail_agent:name(folder), ":  ", msg_seq));
    else
      player:notify(tostr("No messages to ", do == "expunge_rmm" ? "expunge from " | "restore to ", $mail_agent:name(folder)));
    endif
  endverb

  verb "@send" (any any any) owner: #2 flags: "rxd"
    if (args && args[1] == "to")
      args = listdelete(args, 1);
    endif
    subject = {};
    for a in (args)
      if ((i = index(a, "=")) > 3 && index("subject", a[1..i - 1]) == 1)
        args = setremove(args, a);
        a[1..i] = "";
        subject = {a};
      endif
    endfor
    $mail_editor:invoke(args, verb, @subject);
  endverb

  verb "@answer @repl*y" (any any any) owner: #2 flags: "rd"
    "@answer <msg> [on *<recipient>] [<flags>...]";
    set_task_perms(who = valid(caller_perms()) ? caller_perms() | player);
    if (p = this:parse_mailread_cmd(verb, args, "cur", "on", 1))
      if ($seq_utils:size(p[2]) != 1)
        player:notify("You can only answer *one* message at a time.");
      elseif (LIST != typeof(flags_replytos = $mail_editor:check_answer_flags(@p[4])))
        player:notify_lines({tostr("Usage:  ", verb, " [message-# [on <recipient>]] [flags...]"), "where flags include any of:", "  all        reply to everyone", "  sender     reply to sender only", "  include    include the original message in your reply", "  noinclude  don't include the original in your reply"});
      else
        this:set_current_folder(p[1]);
        $mail_editor:invoke(2, verb, p[1]:messages_in_seq(p[2])[1][2], @flags_replytos);
      endif
    endif
  endverb

  verb "@forward" (any any any) owner: #2 flags: "rxd"
    "@forward <msg> [on *<recipient>] to <recipient> [<recipient>...]";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    if (!(p = this:parse_mailread_cmd(verb, args, "", "on", 1)))
      "...lose...";
      return;
    elseif ($seq_utils:size(sequence = p[2]) != 1)
      player:notify("You can only forward *one* message at a time.");
      return;
    elseif (length(p[4]) < 2 || p[4][1] != "to")
      player:notify(tostr("Usage:  ", verb, " [<message>] [on <folder>] to <recip>..."));
      return;
    endif
    recips = {};
    for rs in (listdelete(p[4], 1))
      if ($mail_agent:match_failed(r = $mail_agent:match_recipient(rs), rs))
        return;
      endif
      recips = {@recips, r};
    endfor
    this:set_current_folder(folder = p[1]);
    m = folder:messages_in_seq(sequence)[1];
    msgnum = m[1];
    msgtxt = m[2];
    from = msgtxt[2];
    if (msgtxt[4] != " ")
      subject = tostr("[", from, ":  ", msgtxt[4], "]");
    elseif ((h = "" in msgtxt) && h < length(msgtxt))
      subject = tostr("[", from, ":  `", (msgtxt[h + 1])[1..min(20, $)], "']");
    else
      subject = tostr("[", from, "]");
    endif
    result = $mail_agent:send_message(player, recips, subject, $mail_agent:to_text(@msgtxt));
    if (!result)
      player:notify(tostr(result));
    elseif (result[1])
      player:notify(tostr("Message ", msgnum, @folder == this ? {} | {" on ", $mail_agent:name(folder)}, " @forwarded to ", $mail_agent:name_list(@listdelete(result, 1)), "."));
    else
      player:notify("Message not sent.");
    endif
  endverb

  verb "@gripe" (any any any) owner: #2 flags: "rd"
    $mail_editor:invoke($gripe_recipients, "@gripe", "@gripe: " + argstr);
  endverb

  verb "@typo @bug @suggest*ion @idea @comment" (any any any) owner: #2 flags: "rd"
    subject = tostr($string_utils:capitalize(verb[2..$]), ":  ", (loc = this.location).name, "(", loc, ")");
    if (this != player)
      return E_PERM;
    elseif (argstr)
      result = $mail_agent:send_message(this, {loc.owner}, subject, argstr);
      if (result && result[1])
        player:notify(tostr("Your ", verb, " sent to ", $mail_agent:name_list(@listdelete(result, 1)), ".  Input is appreciated, as always."));
      else
        player:notify(tostr("Huh?  This room's owner (", loc.owner, ") is invalid?  Tell a wizard..."));
      endif
      return;
    elseif (!($object_utils:isa(loc, $room) && loc.free_entry))
      player:notify_lines({tostr("You need to make it a one-liner, i.e., `", verb, " something or other'."), "This room may not let you back in if you go to the Mail Room."});
    elseif ($object_utils:isa(loc, $generic_editor))
      player:notify_lines({tostr("You need to make it a one-liner, i.e., `", verb, " something or other'."), "Sending you to the Mail Room from an editor is usually a bad idea."});
    else
      $mail_editor:invoke({tostr(loc.owner)}, verb, subject);
    endif
    if (verb == "@bug")
      player:notify("For a @bug report, be sure to mention exactly what it was you typed to trigger the error...");
    endif
  endverb

  verb "@skip" (any any any) owner: #2 flags: "rd"
    "@skip [*<folder/mailing_list>...]";
    "  sets your last-read time for the given lists to now, indicating your";
    "  disinterest in any new messages that might have appeared recently.";
    set_task_perms(player);
    current_folder = this:current_folder();
    for a in (args || {0})
      if (a ? $mail_agent:match_failed(folder = $mail_agent:match_recipient(a), a) | (folder = this:current_folder()))
        "...bogus folder name, done...  No, try anyway.";
        if (this:kill_current_message(this:my_match_object(a)))
          player:notify("Invalid folder, but found it subscribed anyway.  Removed.");
        endif
      else
        lseq = folder:length_all_msgs();
        unread = (n = this:get_current_message(folder)) ? folder:length_date_gt(n[2]) | lseq;
        this:set_current_message(folder, lseq && folder:messages_in_seq({lseq, lseq + 1})[1][1], time());
        player:notify(tostr(unread ? tostr("Ignoring ", unread) | "No", " unread message", unread != 1 ? "s" | "", " on ", $mail_agent:name(folder)));
        if (current_folder == folder)
          this:set_current_folder(this);
        endif
      endif
    endfor
  endverb

  verb "@subscribe*-quick @unsubscribed*-quick" (any any any) owner: #2 flags: "rd"
    "@subscribe *<folder/mailing_list> [with notification] [before|after *<folder>]";
    "  causes you to be notified when new mail arrives on this list";
    "@subscribe";
    "  just lists available mailing lists.";
    "@unsubscribed";
    "  prints out available mailing lists you aren't already subscribed to.";
    "@subscribe-quick and @unsubscribed-quick";
    "  prints out same as above except without mail list descriptions, just names.";
    set_task_perms(player);
    quick = 0;
    if (qi = index(verb, "-q"))
      verb = verb[1..qi - 1];
      quick = 1;
    endif
    fname = {@args, 0}[1];
    if (!fname)
      ml = $list_utils:slice((this.current_message)[3..$]);
      all_mlists = {@$mail_agent.contents, @this.mail_lists};
      if (length(all_mlists) > 50 && !$command_utils:yes_or_no(tostr("There are ", length(all_mlists), " mailing lists.  Are you sure you want the whole list?")))
        return player:tell("OK, aborting.");
      endif
      for c in (all_mlists)
        $command_utils:suspend_if_needed(0);
        if (c:is_usable_by(this) || c:is_readable_by(this) && (verb != "@unsubscribed" || !(c in ml)))
          c:look_self(quick);
        endif
      endfor
      player:notify(tostr("-------- end of ", verb, " -------"));
      return;
    elseif (verb == "@unsubscribed")
      player:notify("@unsubscribed does not take arguments.");
      return;
    elseif ($mail_agent:match_failed(folder = $mail_agent:match_recipient(fname), fname))
      return;
    elseif (folder == this)
      player:notify("You don't need to @subscribe to yourself");
      return;
    elseif ($object_utils:isa(folder, $mail_recipient) ? !folder:is_readable_by(this) | !$perm_utils:controls(this, folder))
      player:notify("That mailing list is not readable by you.");
      return;
    endif
    notification = this in folder.mail_notify;
    i = 0;
    beforeafter = 0;
    while (length(args) >= 2)
      if (length(args) < 3)
        player:notify(args[2] + " what?");
        return;
      elseif (args[2] in {"with", "without"})
        with = args[2] == "with";
        if (index("notification", args[3]) != 1)
          player:notify(tostr("with ", args[3], "?"));
          return;
        elseif (!$object_utils:isa(folder, $mail_recipient))
          player:notify(tostr("You cannot use ", verb, " to change mail notification from a non-$mail_recipient."));
        elseif (!with == !notification)
          "... nothing to do...";
        elseif (with)
          if (this in folder:add_notify(this))
            notification = 1;
          else
            player:notify("This mail recipient does not allow immediate notification.");
          endif
        else
          folder:delete_notify(this);
          notification = 0;
        endif
      elseif (args[2] in {"before", "after"})
        if (beforeafter)
          player:notify(args[2] == beforeafter ? tostr("two `", beforeafter, "'s?") | "Only use one of `before' or `after'");
          return;
        elseif ($mail_agent:match_failed(other = $mail_agent:match_recipient(args[3]), args[3]))
          return;
        elseif (other == this)
          i = 2;
        elseif (!(i = $list_utils:iassoc(other, this.current_message)))
          player:notify(tostr("You aren't subscribed to ", $mail_agent:name(other), "."));
          return;
        endif
        beforeafter = args[2];
        i = i - (beforeafter == "before");
        if (this:mail_option("rn_order") != "fixed")
          player:notify("Warning:  Do `@mail-option rn_order=fixed' if you do not want your @rn listing reordered when you next login.");
        endif
      endif
      args[2..3] = {};
    endwhile
    this:make_current_message(folder, @i ? {i} | {});
    len = folder:length_all_msgs();
    player:notify(tostr($mail_agent:name(folder), " has ", len, " message", len == 1 ? "" | "s", ".", notification ? "  You will be notified immediately when new messages are posted." | "  Notification of new messages will be printed when you connect."));
    this:set_current_folder(folder);
  endverb

  verb mail_catch_up (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller == this ? this.owner | caller_perms());
    this:set_current_folder(this);
    dates = new_cm = head = {};
    sort = this:mail_option("rn_order") || "read";
    for n in (this.current_message)
      $command_utils:suspend_if_needed(0);
      if (typeof(n) != LIST)
        head = {@head, n};
      elseif ($object_utils:isa(folder = n[1], $mail_recipient) && folder:is_readable_by(this))
        "...set current msg to be the last one you could possibly have read.";
        if (n[3] < folder.last_msg_date)
          i = folder:length_date_le(n[3]);
          n[2] = i && folder:messages_in_seq(i)[1];
        endif
        if (sort == "fixed")
          new_cm = {n, @new_cm};
        elseif (sort == "send")
          j = $list_utils:find_insert(dates, folder.last_msg_date - 1);
          dates = listinsert(dates, folder.last_msg_date, j);
          new_cm = listinsert(new_cm, n, j);
        else
          new_cm = listappend(new_cm, n, $list_utils:iassoc_sorted(n[3] - 1, new_cm, 3));
        endif
      endif
    endfor
    this.current_message = {@head, @$list_utils:reverse(new_cm)};
  endverb

  verb "@rn check_mail_lists @subscribed @rn-full" (none none none) owner: #2 flags: "rxd"
    set_task_perms(caller == this ? this.owner | caller_perms());
    which = {};
    cm = this.current_message;
    cm[1..2] = verb == "@rn" || verb == "@rn-full" ? {{this, @cm[1..2]}} | {};
    all = verb == "@subscribed";
    fast = this:mail_option("fast_check") && verb != "@rn-full";
    for n in (cm)
      rcpt = n[1];
      if (rcpt == $news)
        "... $news is handled separately ...";
      elseif ($mail_agent:is_recipient(rcpt))
        if (fast)
          if (rcpt == this)
            nmsgs = (m = this.messages) && m[length(m)][2][1] > n[3] ? $maxint | 0;
          else
            try
              nmsgs = n[1].last_msg_date > n[3] ? $maxint | 0;
            except (E_PERM, E_PROPNF)
              player:notify(tostr("Bogus recipient ", rcpt, " removed from .current_message."));
              this.current_message = setremove(this.current_message, n);
              nmsgs = 0;
            endtry
          endif
        else
          nmsgs = n[1]:length_date_gt(n[3]);
        endif
        if (nmsgs || all)
          which = {@which, {n[1], nmsgs}};
        endif
      else
        player:notify(tostr("Bogus recipient ", rcpt, " removed from .current_message."));
        this.current_message = setremove(this.current_message, n);
      endif
      $command_utils:suspend_if_needed(0);
    endfor
    if (which)
      player:notify(tostr(verb == "@subscribed" ? "You are subscribed to the following" | "There is new activity on the following", length(which) > 1 ? " lists:" | " list:"));
      for w in (which)
        name = w[1] == this ? " me" | $mail_agent:name(w[1]);
        player:notify(tostr($string_utils:left("    " + name, 40), " ", w[2] == $maxint ? "has" | w[2], " new message", w[2] == 1 ? "" | "s"));
        $command_utils:suspend_if_needed(0);
      endfor
      if (verb != "check_mail_lists")
        player:notify("-- End of listing");
      endif
    elseif (verb == "@rn" || verb == "@rn-full")
      player:notify("No new activity on any of your lists.");
    elseif (verb == "@subscribed")
      player:notify("You aren't subscribed to any mailing lists.");
    endif
    return which;
  endverb

  verb mail_option (this none this) owner: #2 flags: "rxd"
    ":mail_option(name)";
    "Returns the value of the specified mail option";
    if (caller in {this, $mail_editor, $mail_agent} || $perm_utils:controls(caller_perms(), this))
      return $mail_options:get(this.mail_options, args[1]);
    else
      return E_PERM;
    endif
  endverb

  verb "@unsub*scribe" (any any any) owner: #2 flags: "rd"
    "@unsubscribe [*<folder/mailing_list> ...]";
    "entirely removes the record of your current message for the named folders,";
    "indicating your disinterest in anything that might appear there in the future.";
    set_task_perms(player);
    unsubscribed = {};
    current_folder = this:current_folder();
    for a in (args || {0})
      if (a != 0)
        folder = $mail_agent:match_recipient(a);
        if (folder == $failed_match)
          folder = this:my_match_object(a);
        endif
      else
        folder = current_folder;
      endif
      if (!valid(folder))
        "...bogus folder name...  try removing it anyway.";
        if (this:kill_current_message(folder))
          player:notify("Invalid folder, but found it subscribed anyway.  Removed.");
        else
          $mail_agent:match_failed(folder, a);
        endif
      elseif (folder == this)
        player:notify(tostr("You can't ", verb, " yourself."));
      else
        if (!this:kill_current_message(folder))
          player:notify(tostr("You weren't subscribed to ", $mail_agent:name(folder)));
          if ($object_utils:isa(folder, $mail_recipient))
            result = folder:delete_notify(this);
            if (typeof(result) == LIST && result[1] == this)
              player:notify("Removed you from the mail notifications list.");
            endif
          endif
        else
          unsubscribed = {@unsubscribed, folder};
          if ($object_utils:isa(folder, $mail_recipient))
            folder:delete_notify(this);
          endif
        endif
      endif
    endfor
    if (unsubscribed)
      player:notify(tostr("Forgetting about ", $string_utils:english_list($list_utils:map_arg($mail_agent, "name", unsubscribed))));
      if (current_folder in unsubscribed)
        this:set_current_folder(this);
      endif
    endif
  endverb

  verb send_self_netmail (this none this) owner: #2 flags: "rxd"
    ":send_self_netmail(msg [ ,from ])";
    "return 0 if successful, otherwise error.";
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (error = $network:invalid_email_address($wiz_utils:get_email_address(this)))
      return "Invalid email address: " + error;
    else
      msg = args[1];
      if (length(args) > 1)
        from = args[2];
        this:notify(tostr("Receiving mail from ", from:title(), " (", from, ") and forwarding it to your .email_address."));
      endif
      oplayer = player;
      player = this;
      error = $network:sendmail($wiz_utils:get_email_address(this), @msg);
      if (error && length(args) > 1)
        this:notify(tostr("Mail sending failed: ", error));
      endif
      player = oplayer;
      return error;
    endif
  endverb

  verb "@netforw*ard" (any any any) owner: #2 flags: "rxd"
    "@netforward <msg>...                  -- as in help on @netforward";
    "@netforward <msg>... on *<recipient>  -- netforwards messages on recipient.";
    "This command forwards mail-messages to your registered email-address.";
    if (player != this)
      return player:tell(E_PERM);
    endif
    if (reason = $network:email_will_fail(email = $wiz_utils:get_email_address(player)))
      return player:notify(tostr("Cannot forward mail to your email address: ", reason));
    endif
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    if (p = player:parse_mailread_cmd(verb, args, "", "on"))
      player:set_current_folder(folder = p[1]);
      msg_seq = p[2];
      folderstr = folder == player ? "" | tostr(" from ", $mail_agent:name(folder));
      if (!this:mail_option("expert_netfwd") && !$command_utils:yes_or_no(tostr("You are about to forward ", seq_size = $seq_utils:size(msg_seq), " message(s)", folderstr, " to your registered email-address, ", email, ".  Continue?")))
        player:notify(tostr("@Netforward cancelled."));
        return;
      endif
      player:notify("Attempting to send network mail...");
      player._mail_task = task_id();
      multiple_vals = this:format_for_netforward(folder:messages_in_seq(msg_seq), folderstr);
      netmail = multiple_vals[1];
      header = multiple_vals[2];
      reason = player:send_self_netmail({header, @netmail});
      player:notify(reason == 0 ? tostr("@netforward of ", header, " completed.") | tostr("@netforward failed: ", reason, "."));
    endif
  endverb

  verb "@@sendmail" (any any any) owner: #2 flags: "rd"
    "Syntax: @@sendmail";
    "This is intended for use with client editors.  You probably don't want to try using this command manually.";
    "Reads a formatted mail message, extracts recipients, subject line and/or reply-to header and sends message without going to the mailroom.  Example:";
    "";
    "@@send";
    "To: Rog (#4292)";
    "Subject: random";
    "";
    "first line";
    "second line";
    ".";
    "";
    "Currently, header lines must have the same format as in an actual message.";
    set_task_perms(player);
    if (args)
      player:notify(tostr("The ", verb, " command takes no arguments."));
      $command_utils:read_lines();
      return;
    elseif (this != player)
      player:notify(tostr("You can't use ", this.pp, " ", verb, " verb."));
      $command_utils:read_lines();
      return;
    endif
    msg = $command_utils:read_lines();
    end_head = "" in msg || length(msg) + 1;
    from = this;
    subject = "";
    replyto = "";
    rcpts = {};
    body = msg[end_head + 1..$];
    for i in [1..end_head - 1]
      line = msg[i];
      if (index(line, "Subject:") == 1)
        subject = $string_utils:trim(line[9..$]);
      elseif (index(line, "To:") == 1)
        if (!(rcpts = $mail_agent:parse_address_field(line)))
          player:notify("No recipients found in To: line");
          return;
        endif
      elseif (index(line, "Reply-to:") == 1)
        if (!(replyto = $mail_agent:parse_address_field(line)) && $string_utils:trim(line[10..$]))
          player:notify("No address found in Reply-to: line");
          return;
        endif
      elseif (index(line, "From:") == 1)
        "... :send_message() bombs if designated sender != player ...";
        if (!(from = $mail_agent:parse_address_field(line)))
          player:notify("No sender found in From: line");
          return;
        elseif (length(from) > 1)
          player:notify("Multiple senders?");
          return;
        endif
        from = from[1];
      elseif (i = index(line, ":"))
        player:notify(tostr("Unknown header \"", line[1..i], "\""));
        return;
      else
        player:notify("Blank line must separate headers from body.");
        return;
      endif
    endfor
    if (!rcpts)
      player:notify("No To: line found.");
    elseif (!(subject || body))
      player:notify("Blank message not sent.");
    else
      player:notify("Sending...");
      result = $mail_agent:send_message(from, rcpts, replyto ? {subject, replyto} | subject, body);
      if (e = result && result[1])
        if (length(result) == 1)
          player:notify("Mail actually went to no one.");
        else
          player:notify(tostr("Mail actually went to ", $mail_agent:name_list(@listdelete(result, 1)), "."));
        endif
      else
        player:notify(tostr(typeof(e) == ERR ? e | "Bogus recipients:  " + $string_utils:from_list(result[2])));
        player:notify("Mail not sent.");
      endif
    endif
  endverb

  verb "@keep-m*ail @keepm*ail" (any any any) owner: #2 flags: "rd"
    "@keep-mail [<msg-sequence>|none] [on <recipient>]";
    "marks the indicated messages as `kept'.";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    if (!args)
      return player:notify("Usage:  @keep-mail [<msg-sequence>|none] [on <recipient>]");
    elseif (args[1] == "none")
      args[1..1] = {};
      if (!(pfs = this:parse_folder_spec(verb, args, "on", 0)))
        return;
      elseif (pfs[2])
        player:notify(tostr(verb, " <message-sequence> or `none', but not both."));
        return;
      endif
      this:set_current_folder(folder = pfs[1]);
      if (e = folder:keep_message_seq({}))
        player:notify(tostr("Messages on ", $mail_agent:name(folder), " are no longer marked as kept."));
      else
        player:notify(tostr(e));
      endif
      return;
    elseif (p = this:parse_mailread_cmd(verb, args, "", "on"))
      if ((folder = p[1]) != this)
        "... maybe I'll take this clause out some day...";
        player:notify(tostr(verb, " can only be used on your own mail collection."));
        return;
      endif
      this:set_current_folder(folder);
      if (e = folder:keep_message_seq(msg_seq = p[2]))
        player:notify(tostr("Message", match(e, "[.,]") ? "s " | " ", e, " now marked as kept."));
      elseif (typeof(e) == ERR)
        player:notify(tostr(e));
      else
        player:notify(tostr((seq_size = $seq_utils:size(msg_seq)) == 1 ? "That message is" | "Those messages are", " already marked as kept."));
      endif
    endif
  endverb

  verb my_match_recipient (this none this) owner: #2 flags: "rxd"
    ":my_match_recipient(string) => matches string against player's private mailing lists.";
    if (!(string = args[1]))
      return $nothing;
    elseif (string[1] == "*")
      string = string[2..$];
    endif
    return $string_utils:match(string, this.mail_lists, "aliases");
  endverb

  verb expire_old_messages (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    else
      seq = this:expirable_msg_seq();
      if (seq)
        this:rm_message_seq(seq);
        return this:expunge_rmm();
      else
        return 0;
      endif
    endif
  endverb

  verb msg_full_text (this none this) owner: #2 flags: "rxd"
    ":msg_full_text(@msg) => list of strings.";
    "msg is a mail message (in the usual transmission format).";
    "display_seq_full calls this to obtain the actual list of strings to display.";
    return player:msg_text(@args);
    "default is to leave it up to the player how s/he wants it to be displayed.";
  endverb

  verb "@resend" (any any any) owner: #2 flags: "rd"
    "@resend <msg> [on *<recipient>] to <recipient> [<recipient>...]";
    set_task_perms(valid(caller_perms()) ? caller_perms() | player);
    "...";
    "... parse command...";
    "...";
    if (!(p = this:parse_mailread_cmd(verb, args, "", "on", 1)))
      "...lose...";
      return;
    elseif ($seq_utils:size(sequence = p[2]) != 1)
      player:notify("You can only resend *one* message at a time.");
      return;
    elseif (length(p[4]) < 2 || p[4][1] != "to")
      player:notify(tostr("Usage:  ", verb, " [<message>] [on <folder>] to <recip>..."));
      return;
    endif
    recips = {};
    for rs in (listdelete(p[4], 1))
      if ($mail_agent:match_failed(r = $mail_agent:match_recipient(rs), rs))
        return;
      endif
      recips = {@recips, r};
    endfor
    this:set_current_folder(folder = p[1]);
    "...";
    "... retrieve original message...";
    "...";
    {msgnum, msgtxt} = folder:messages_in_seq(sequence)[1];
    if (forward_style = this:mail_option("resend_forw"))
      "...message will be from player...";
      pmh = $mail_agent:parse_misc_headers(msgtxt, "Reply-To", "Original-Date", "Original-From");
      orig_from = pmh[3][3] || msgtxt[2];
    else
      "...message will be from author...";
      pmh = $mail_agent:parse_misc_headers(msgtxt, "Reply-To", "Original-Date", "Original-From", "Resent-By", "Resent-To");
      orig_from = pmh[3][3];
      from = $mail_agent:parse_address_field(msgtxt[2])[1];
      to = $mail_agent:parse_address_field(msgtxt[3]);
    endif
    "...";
    "... report bogus headers...";
    "...";
    if (bogus = pmh[2])
      player:notify("Bogus headers stripped from original message:");
      for b in (bogus)
        player:notify("  " + b);
      endfor
      if (!$command_utils:yes_or_no("Continue?"))
        player:notify("Message not resent.");
        return;
      endif
    endif
    "...";
    "... subject, replyto, original-date, original-from ...";
    "...";
    hdrs = {msgtxt[4], pmh[3][1], {"Original-Date", pmh[3][2] || ctime(msgtxt[1])}, @orig_from ? {{"Original-From", orig_from}} | {}, @pmh[1]};
    "...";
    "... send it ...";
    "...";
    if (forward_style)
      result = $mail_agent:send_message(player, recips, hdrs, pmh[4]);
    else
      "... resend inserts resent-to and resent-by...";
      result = $mail_agent:resend_message(player, recips, from, to, hdrs, pmh[4]);
    endif
    "...";
    "... report outcome...";
    "...";
    if (!result)
      player:notify(tostr(result));
    elseif (result[1])
      player:notify(tostr("Message ", msgnum, @folder == this ? {} | {" on ", $mail_agent:name(folder)}, " @resent to ", $mail_agent:name_list(@listdelete(result, 1)), "."));
    else
      player:notify("Message not resent.");
    endif
  endverb

  verb expirable_msg_seq (this none this) owner: #2 flags: "rxd"
    "Return a sequence indicating the expirable messages for this player.";
    set_task_perms(caller_perms());
    if (!$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (!(curmsg = this:get_current_message(this)))
      "No messages!  Don't even try.";
      return {};
    elseif (0 >= (period = this:mail_option("expire") || $mail_agent.player_expire_time))
      "...no expiration allowed here...";
      return {};
    else
      return $seq_utils:remove(this:unkept_msg_seq(), 1 + this:length_date_le(min(time() - period, curmsg[2] - 86400)));
      "... the 86400 is pure fudge...";
    endif
  endverb

  verb format_for_netforward (this none this) owner: #2 flags: "rxd"
    "Takes a message sequence (the actual messages, not just the sequence describing it) and grovels over it filling text etc.  Returns a two valued list: {formatted message, header for same}";
    set_task_perms(caller_perms());
    {message_seq, folderstr} = args;
    netmail = {};
    linelen = this:linelen();
    maxmsg = minmsg = 0;
    for msg in (message_seq)
      minmsg = minmsg ? min(msg[1], minmsg) | msg[1];
      maxmsg = maxmsg ? max(msg[1], maxmsg) | msg[1];
      lines = {tostr("Message ", msg[1], folderstr, ":"), tostr("Date:     ", ctime(msg[2][1])), "From:     " + msg[2][2], "To:       " + msg[2][3], @length(subj = msg[2][4]) > 1 ? {"Subject:  " + subj} | {}};
      for line in ((msg[2])[5..$])
        if (typeof(line) != STR)
          "I don't know how this can happen, but apparently non-strings can end up in the mail message.  So, cope.";
          line = tostr(line);
        endif
        lines = {@lines, @$generic_editor:fill_string(line, linelen)};
        $command_utils:suspend_if_needed(0);
      endfor
      netmail = {@netmail, @lines, "", "--------------------------", "", ""};
    endfor
    header = tostr($network.MOO_name, " Message(s) ", minmsg, @minmsg != maxmsg ? {" - ", maxmsg} | {}, folderstr);
    return {netmail, header};
  endverb

  verb format_for_netforward_debug (this none this) owner: #2 flags: "rxd"
    "Takes a message sequence (the actual messages, not just the sequence describing it) and grovels over it filling text etc.  Returns a two valued list: {formatted message, header for same}";
    set_task_perms(caller_perms());
    {message_seq, folderstr} = args;
    netmail = {};
    linelen = this:linelen();
    maxmsg = minmsg = 0;
    for msg in (message_seq)
      minmsg = minmsg ? min(msg[1], minmsg) | msg[1];
      maxmsg = maxmsg ? max(msg[1], maxmsg) | msg[1];
      lines = {tostr("Message ", msg[1], folderstr, ":"), tostr("Date:     ", ctime(msg[2][1])), "From:     " + msg[2][2], "To:       " + msg[2][3], @length(subj = msg[2][4]) > 1 ? {"Subject:  " + subj} | {}};
      for line in ((msg[2])[5..$])
        if (typeof(line) != STR)
          "I don't know how this can happen, but apparently non-strings can end up in the mail message.  So, cope.";
          line = tostr(line);
        endif
        lines = {@lines, @$generic_editor:fill_string(line, linelen)};
        $command_utils:suspend_if_needed(0);
      endfor
      netmail = {@netmail, @lines, "", "--------------------------", "", ""};
    endfor
    header = tostr($network.MOO_name, " Message(s) ", minmsg, @minmsg != maxmsg ? {" - ", maxmsg} | {}, folderstr);
    return {netmail, header};
  endverb

  verb "@nn" (none none none) owner: #2 flags: "rxd"
    "@nn  -- reads the first new message on the first mail_recipient (in .current_message) where new mail exists.";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    cm = this.current_message;
    cm[1..2] = {{this, @cm[1..2]}};
    for n in (cm)
      if ($mail_agent:is_recipient(n[1]))
        if (new = n[1]:length_date_gt(n[3]))
          next = n[1]:length_all_msgs() - new + 1;
          this:set_current_folder(folder = n[1]);
          this._mail_task = task_id();
          cur = folder:display_seq_full({next, next + 1}, tostr("Message %d", " on ", $mail_agent:name(folder), ":"));
          this:set_current_message(folder, @cur);
          return;
        endif
      else
        player:notify(tostr("Bogus recipient ", n[1], " removed from .current_message."));
        this.current_message = setremove(this.current_message, n);
      endif
    endfor
    player:tell("No News (is good news)");
  endverb

  verb "@unread" (any any any) owner: #2 flags: "rd"
    "@unread <msg> [on *<recipient>]  -- resets last-read-date for recipient to just before the first of the indicated messages.";
    set_task_perms(player);
    if (p = this:parse_mailread_cmd("@unread", args, "cur", "on"))
      this:set_current_folder(folder = p[1]);
      msg_ord = $seq_utils:first(msg_seq = p[2]);
      msgdate = folder:messages_in_seq(msg_ord)[2][1] - 1;
      if (!(cm = this:get_current_message(folder)) || cm[2] < msgdate)
        player:notify("Already unread.");
      else
        if (folder == this)
          this.current_message[2] = msgdate - 1;
        else
          "this:kill_current_message(folder);";
          this:set_current_message(folder, cm[1], min(cm[2], msgdate), 1);
        endif
        folder:display_seq_headers({msg_ord, msg_ord + 1}, cm[1], msgdate);
      endif
    endif
  endverb

  verb "@refile @copym*ail" (any any any) owner: #2 flags: "rd"
    "@refile/@copym*ail <msg-sequence> [on <recipient>] to <recipient>";
    "@refile will delete the messages from the source folder.  @copym does not.";
    "I'm not happy with this one, yet...";
    set_task_perms(player);
    if (!(p = this:parse_mailread_cmd("@refile", args, "cur", "on", 1)))
      "...lose...";
    elseif (length(p[4]) != 2 || p[4][1] != "to")
      player:notify(tostr("Usage:  ", verb, " [<message numbers>] [on <folder>] to <folder>"));
    elseif ($mail_agent:match_failed(dest = $mail_agent:match_recipient(p[4][2]), p[4][2]))
      "...bogus destination folder...";
    else
      source = p[1];
      msg_seq = p[2];
      for m in (source:messages_in_seq(msg_seq))
        if (!(e = dest:receive_message(m[2], source)))
          player:notify(tostr("Copying msg. ", m[1], ":  ", e));
          return;
        endif
        $command_utils:suspend_if_needed(0);
      endfor
      if (refile = verb == "@refile")
        if (typeof(e = source:rm_message_seq(msg_seq)) == ERR)
          player:notify(tostr("Deleting from ", source, ":  ", e));
        endif
      endif
      count = tostr(n = $seq_utils:size(msg_seq), " message", n == 1 ? "" | "s");
      fname = source == this ? "" | tostr(is_player(source) ? " from " | " from *", source.name, "(", source, ")");
      suffix = tostr(is_player(dest) ? " to " | " to *", dest.name, "(", dest, ").");
      player:notify(tostr(refile ? "Refiled " | "Copied ", count, fname, suffix));
    endif
  endverb

  verb "@quickr*eply @qreply" (any any any) owner: #2 flags: "rd"
    "@qreply <msg> [on *<recipient>] [<flags>...]";
    "like @reply only, as in @qsend, we prompt for the message text using ";
    "$command_utils:read_lines() rather than invoking the $mail_editor.";
    set_task_perms(who = valid(cp = caller_perms()) ? cp | player);
    if (!(p = this:parse_mailread_cmd(verb, args, "cur", "on", 1)))
      "...garbled...";
    elseif ($seq_utils:size(p[2]) != 1)
      player:notify("You can only answer *one* message at a time.");
    elseif (LIST != typeof(flags_replytos = $mail_editor:check_answer_flags("noinclude", @p[4])))
      player:notify_lines({tostr("Usage:  ", verb, " [message-# [on <recipient>]] [flags...]"), "where flags include any of:", "  all        reply to everyone", "  sender     reply to sender only", tostr("  include    include the original message in reply (can't do this for ", verb, ")"), "  noinclude  don't include the original in your reply"});
    elseif ("include" in flags_replytos[1])
      player:notify(tostr("Can't include message on a ", verb));
    else
      this:set_current_folder(p[1]);
      if (to_subj = $mail_editor:parse_msg_headers(p[1]:messages_in_seq(p[2])[1][2], flags_replytos[1]))
        player:notify(tostr("To:       ", $mail_agent:name_list(@to_subj[1])));
        if (to_subj[2])
          player:notify(tostr("Subject:  ", to_subj[2]));
        endif
        if (replytos = flags_replytos[2])
          player:notify(tostr("Reply-to: ", $mail_agent:name_list(@replytos)));
        endif
        hdrs = {to_subj[2], replytos || {}};
        player:notify("Enter lines of message:");
        message = $command_utils:read_lines_escape((active = player in $mail_editor.active) ? {} | {"@edit"}, {tostr("You are composing mail to ", $mail_agent:name_list(@to_subj[1]), "."), @active ? {} | {"Type `@edit' to take this into the mail editor."}});
        if (typeof(message) == ERR)
          player:notify(tostr(message));
        elseif (message[1] == "@edit")
          $mail_editor:invoke(1, verb, to_subj[1], @hdrs, message[2]);
        elseif (!message[2])
          player:notify("Blank message not sent.");
        else
          result = $mail_agent:send_message(this, to_subj[1], hdrs, message[2]);
          if (result && result[1])
            player:notify(tostr("Message sent to ", $mail_agent:name_list(@listdelete(result, 1)), "."));
          else
            player:notify("Message not sent.");
          endif
        endif
      endif
    endif
  endverb

  verb "@mail-all-new*-mail" (none none none) owner: #2 flags: "rxd"
    "@mail-all-new-mail";
    " Prints headers for all new mail on every mail-recipient mentioned in .current_message.";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    cm = this.current_message;
    cm[1..2] = {{this, @cm[1..2]}};
    this._mail_task = task_id();
    nomail = 1;
    new_cms = {};
    for f in (cm)
      if (!($object_utils:isa(folder = f[1], $player) || $object_utils:isa(folder, $mail_recipient)))
        player:notify(tostr(folder, " is neither a $player nor a $mail_recipient"));
      elseif (typeof(flen = folder:length_all_msgs()) == ERR)
        player:notify(tostr($mail_agent:name(folder), " ", flen));
      elseif (msg_seq = $seq_utils:range(folder:length_date_le(f[3]) + 1, flen))
        nomail = 0;
        player:notify("===== " + $string_utils:left(tostr($mail_agent:name(folder), " (", s = $seq_utils:size(msg_seq), " message", s == 1 ? ") " | "s) "), 40, "="));
        folder:display_seq_headers(msg_seq, @f[2..3]);
        player:notify("");
        $command_utils:suspend_if_needed(2);
      endif
    endfor
    if (nomail)
      player:notify("You don't have any new mail anywhere.");
    else
      player:notify("===== " + $string_utils:left("End of new mail ", 40, "="));
    endif
  endverb

  verb "@read-all-new*-mail @ranm" (any none none) owner: #2 flags: "rxd"
    "@read-all-new-mail [yes]";
    " Prints all new mail on every mail-recipient mentioned in .current_message";
    " Generally this will spam you into next Tuesday.";
    " You will be queried for whether you want your last-read dates updated";
    "   but you can specify \"yes\" on the command line to suppress this.";
    "   If you do so, last-read dates will be updated after each folder read.";
    set_task_perms(valid(cp = caller_perms()) ? cp | player);
    noconfirm = args && args[1];
    if (noconfirm && noconfirm != "yes" && noconfirm != "no")
      player:notify("Unexpected argument(s): " + argstr);
      return;
    endif
    cm = this.current_message;
    cm[1..2] = {{this, @cm[1..2]}};
    this._mail_task = task_id();
    nomail = 1;
    new_cms = {};
    for f in (cm)
      if (!($object_utils:isa(folder = f[1], $player) || $object_utils:isa(folder, $mail_recipient)))
        player:notify(tostr(folder, " is neither a $player nor a $mail_recipient"));
      elseif (typeof(flen = folder:length_all_msgs()) == ERR)
        player:notify(tostr($mail_agent:name(folder), " ", flen));
      elseif (msg_seq = $seq_utils:range(folder:length_date_le(f[3]) + 1, flen))
        nomail = 0;
        player:notify("===== " + $string_utils:left(tostr($mail_agent:name(folder), " (", s = $seq_utils:size(msg_seq), " message", s == 1 ? ") " | "s) "), 40, "="));
        player:notify("");
        if (cur = folder:display_seq_full(msg_seq, tostr("Message %d", folder == this ? "" | " on " + $mail_agent:name(folder), ":")))
          if (noconfirm == "yes")
            this:set_current_message(folder, @cur);
            this:set_current_folder(folder);
          else
            new_cms = {@new_cms, {folder, @cur}};
          endif
          player:notify("");
        endif
      endif
      $command_utils:suspend_if_needed(1);
      this._mail_task = task_id();
    endfor
    if (nomail)
      player:notify("You don't have any new mail anywhere.");
    elseif (player:notify("===== " + $string_utils:left("End of new mail ", 40, "=")) || (noconfirm ? noconfirm == "yes" | $command_utils:yes_or_no("Did you get all of that?")))
      for n in (new_cms)
        this:set_current_message(@n);
        this:set_current_folder(n[1]);
      endfor
      player:notify("Last-read-dates updated");
    else
      player:notify("Last-read-dates not updated");
    endif
  endverb

  verb "@quick*send @qsend" (any any any) owner: #2 flags: "rd"
    "Syntax: @quicksend <recipients(s)> [subj=<text>] [<message>]";
    "Sends the recipients(s) a quick message, wit{out having to go to the mailroom. If there is more than one recipients, place them all in quotes. If the subj contains spaces, place it in quotes.";
    "To put line breaks in the message, use a caret (^).";
    "If no message is given, prompt for lines of message.";
    "Examples:";
    "@quicksend Alice subj=\"Wonderland is neat!\" Have you checked out the Wonderland scenario yet? I think you'd like it!";
    "@quicksend \"Ethel Fred\" Have you seen Lucy around?^--Ricky";
    set_task_perms($object_utils:isa(player, $guest) ? player.owner | player);
    if (!args)
      player:notify(tostr("Usage: ", verb, " <recipients(s)> [subj=<text>] [<message>]"));
      return E_INVARG;
    elseif (this != player)
      player:notify(tostr("You can't use ", this.pp, " @quicksend verb."));
      return E_PERM;
    elseif (!(recipients = $mail_editor:parse_recipients({}, $string_utils:explode(args[1]))))
      return;
    else
      if (length(args) > 1 && ((eq = index(args[2], "=")) && index("subject", (args[2])[1..eq - 1]) == 1))
        subject = $string_utils:trim((args[2])[eq + 1..$]);
        ws = $string_utils:word_start(argstr);
        argstr = argstr[1..ws[1][2]] + argstr[ws[2][2] + 1..$];
        args = listdelete(args, 2);
      else
        subject = "";
      endif
      if (length(args) > 1)
        unbroken = argstr[argstr[1] == "\"" ? length(args[1]) + 4 | length(args[1]) + 2..$] + "^";
        message = {};
        while (unbroken)
          if (i = index(unbroken, "^"))
            message = {@message, unbroken[1..i - 1]};
          endif
          unbroken = unbroken[i + 1..$];
        endwhile
      else
        if (!(subject || player:mail_option("nosubject")))
          player:notify("Subject:");
          subject = $command_utils:read();
        endif
        player:notify("Enter lines of message:");
        message = $command_utils:read_lines_escape((active = player in $mail_editor.active) ? {} | {"@edit"}, {tostr("You are composing mail to ", $mail_agent:name_list(@recipients), "."), @active ? {} | {"Type `@edit' to take this into the mail editor."}});
        if (typeof(message) == ERR)
          player:notify(tostr(message));
          return;
        elseif (message[1] == "@edit")
          $mail_editor:invoke(1, verb, recipients, subject, {}, message[2]);
          return;
        elseif (!(message[2] || subject))
          player:notify("Blank message not sent.");
          return;
        endif
        message = message[2];
      endif
      result = $mail_agent:send_message(this, recipients, subject, message);
      if (result && result[1])
        player:notify(tostr("Message sent to ", $mail_agent:name_list(@listdelete(result, 1)), "."));
      else
        player:notify("Message not sent.");
      endif
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_options = {};
    endif
  endverb

  verb confunc (this none this) owner: #2 flags: "rxd"
    if (valid(cp = caller_perms()) && caller != this && !$perm_utils:controls(cp, this) && caller != #0)
      return E_PERM;
    endif
    this:check_mail();
    this:mail_catch_up();
    this:check_mail_lists();
    pass(@args);
  endverb

  verb "@add-notify" (any at any) owner: #2 flags: "rd"
    "Ideally, in order for one person to be notified that another person has new mail, both the mail recipient and the notification recipient should agree that this is an OK transfer of information.";
    "Usage:  @add-notify me to player";
    "    Sends mail to player saying that I want to be added to their mail notification property.";
    "Usage:  @add-notify player to me";
    "    Makes sure that player wants to be notified, if so, adds them to my .mail_notify property.  (Deletes from temporary record.)";
    if (this == dobj)
      target = $string_utils:match_player(iobjstr);
      if ($command_utils:player_match_failed(target, iobjstr))
        return;
      elseif (this in target.mail_notify[1])
        player:tell("You already receive notifications when ", target.name, " receives mail.");
      elseif (this in target.mail_notify[2])
        player:tell("You already asked to be notified when ", target.name, " receives mail.");
      else
        $mail_agent:send_message(player, {target}, "mail notification request", {tostr($string_utils:nn(this), " would like to receive mail notifications when you get mail."), "Please type:", tostr("  @add-notify ", this.name, " to me"), "if you wish to allow this action."});
        player:tell("Notifying ", $string_utils:nn(target), " that you would like to be notified when ", target.ps, " receives mail.");
        target.mail_notify[2] = setadd(target.mail_notify[2], this);
      endif
    elseif (this == iobj)
      target = $string_utils:match_player(dobjstr);
      if ($command_utils:player_match_failed(target, dobjstr))
        return;
      elseif (target in this.mail_notify[2])
        this.mail_notify[1] = setadd(this.mail_notify[1], target);
        this.mail_notify[2] = setremove(this.mail_notify[2], target);
        player:tell(target.name, " will be notified when you receive mail.");
      else
        player:tell("It doesn't look like ", target.name, " wants to be notified when you receive mail.");
      endif
    else
      player:tell("Usage:  @add-notify me to player");
      player:tell("        @add-notify player to me");
    endif
  endverb

  verb mail_notify (this none this) owner: #2 flags: "rxd"
    if (length(this.mail_notify) > 0 && typeof(this.mail_notify[1]) == LIST)
      return this.mail_notify[1];
    else
      return this.mail_notify;
    endif
  endverb

  verb "@unsend" (any from any) owner: #2 flags: "rd"
    "USAGE: @unsend [message-sequence] from <player>";
    "Attempts to unsend messages you sent to <player>. Per *B:Unsend, messages may not be unsent if they have been read, have been netforwarded to the player (@mail-option +netmail), or if the player has set emself so that mail may not be unsent from em (@mail-option +no_unsend). In addition, mail sent to multiple players may not be unsent unless it can be unsent from each recipient.";
    "";
    "The following message sequences are the only ones allowed:";
    "";
    "  before:<date>    - Strictly before the given date.";
    "  after:<date>     - Strictly after the given date.";
    "  since:<date>     - On or after the given date.";
    "  until:<date>     - On or before the given date.";
    "  subject:<string> - The subject contains the given string.";
    "  body:<string>    - The message body contains the given string.";
    "  last:<number>    - The last <number> messages you sent.";
    "";
    "If you do not specify a sequence, the default sequence stored in @mail-option @unsend will be used.";
    base = dobjstr || this:mail_option(verb) || $mail_agent.("player_default_@unsend");
    if (player != this)
      return player:tell(E_PERM);
    elseif (typeof(base) == STR)
      seq = $string_utils:words(base);
    else
      seq = base;
    endif
    who = $string_utils:match_player(iobjstr);
    fail_msg = "Message(s) were not removed as expected. As per *B:Unsend, I cannot elaborate on why.";
    if ($command_utils:player_match_failed(who, iobjstr))
      return;
    elseif (typeof(res = $mail_options:parse({verb, @seq})) == STR)
      return player:notify(res);
    elseif (who:mail_option("no_unsend") || $object_utils:has_callable_verb(who, "do_unsend") != {$mail_recipient_class})
      "Author's note: I'm not checking for +netmail. The player could have turned it on (or off) later. Netmailed messages are not saved on the player, so they can't be removed, anyway.";
      return player:notify(fail_msg);
    endif
    "The following loop weeds out `last:#' references, which need to be proccessed in a specific way.";
    newseq = otherpeople = {};
    last = 0;
    for x in (seq)
      if (`x[1..5] == "last:" ! ANY')
        last = toint(x[6..$]);
      else
        newseq = {@newseq, x};
      endif
    endfor
    seq = {"new", "unkept:", tostr("from:", player), @newseq};
    if (last > 0)
      seq = {@seq, tostr("last:", last)};
    endif
    ok = who:parse_message_seq(seq, who:current_message());
    if (typeof(ok) != LIST)
      return player:notify(fail_msg);
    endif
    allmsgs = length($seq_utils:tolist(@ok));
    count = missed = 0;
    for position in ($list_utils:reverse($list_utils:range(allmsgs)))
      time = time() + 60;
      ok = position == allmsgs ? ok | who:parse_message_seq(seq, who:current_message());
      if (typeof(ok) == STR || !ok[1])
        break;
      elseif (time() > time)
        player:notify("Due to a mysterious time delay (probably incredible lag), your @unsend command has been aborted. Try again later.");
        count && player:notify(tostr(count, "message", count == 1 ? "" | "s", " were unsent before the command was aborted."));
        return otherpeople && player:notify(tostr("Message(s) were also removed from ", $string_utils:nn(otherpeople), "."));
      endif
      x = $seq_utils:tolist(@ok)[$ - missed];
      ok = {x, x + 1};
      whomail = who.messages;
      bad = 0;
      possible = {};
      "Check if a message was sent to multiple people and set them up for @unsend, too.";
      if ((recips = $mail_agent:parse_address_field((msg = whomail[x][2])[3])) == {who})
        who:do_unsend(ok);
        "Leaving the zombie messages kinda defeats the purpose of @unsend. Since use of @unsend removes any old zombie mail, and since I can't find any nice, tidy way to save the old zombie mail, we're just going to delete the new zombie mail outright. Those who don't like this can set emselves +no_unsend.";
        who.messages_going = {};
        count = count + 1;
        ticks_left() < 5000 || seconds_left() < 2 && suspend(1);
        continue;
      else
        if (ticks_left() / 5000 < length(recips) || seconds_left() < 2)
          suspend(1);
        endif
        "This runs on the principle that the same message text will be sent to each person. If their .messages is in a non-standard format, this will probably bomb. Such people should set themselves +no_unsend, anyway.";
        for y in (setremove(recips, who))
          time = time() + 60;
          if (!is_player(y) || y:mail_option("no_unsend") || $object_utils:has_callable_verb(y, "do_unsend") != {$mail_recipient_class} || typeof(z = y:parse_message_seq({"new", "unkept:"}, y:current_message())) == STR || !z)
            bad = 1;
          elseif (time() > time)
            player:notify("Due to a mysterious time delay (probably incredible lag), your @unsend command has been aborted. Try again later.");
            count && player:notify(tostr(count, "message", count == 1 ? "" | "s", " were unsent before the command was aborted."));
            return otherpeople && player:notify(tostr("Message(s) were also removed from ", $string_utils:nn(otherpeople), "."));
          else
            ymail = y.messages;
            numnum = 0;
            for post in ($seq_utils:tolist(@z))
              if (ymail[post][2] == msg)
                numnum = post;
                break;
              endif
            endfor
            if (!numnum)
              bad = 1;
            else
              z = {numnum, numnum + 1};
            endif
          endif
          possible = bad ? {} | {@possible, {y, z}};
          if (bad)
            break;
          endif
        endfor
      endif
      if (bad)
        missed = missed + 1;
      else
        for foo in ({{who, ok}, @possible})
          {person, sequence} = foo;
          person:do_unsend(sequence);
          "Leaving the zombie messages kinda defeats the purpose of @unsend. Since use of @unsend removes any old zombie mail, and since I can't find any nice, tidy way to save the old zombie mail, we're just going to delete the new zombie mail outright. Those who don't like this can set emselves +no_unsend.";
          person.messages_going = {};
          if (person != who)
            otherpeople = setadd(otherpeople, person);
          endif
        endfor
        count = count + 1;
      endif
    endfor
    if (!count || count != allmsgs)
      player:notify(fail_msg);
    endif
    count && player:notify(tostr(count, " message", count == 1 ? "" | "s", " unsent."));
    otherpeople && player:notify(tostr("Message(s) were also removed from ", $string_utils:nn(otherpeople), "."));
  endverb

  verb do_unsend (this none this) owner: #2 flags: "rxd"
    ":do_unsend(seq) -> Remove the specified messages. Used by @unsend. Cannot be overridden by players or player classes; @unsend won't bother to call the verb.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    return $mail_agent:rm_message_seq(@args);
  endverb

  verb "@annotate*mail" (any any any) owner: #2 flags: "rd"
    "@annotate <msg-sequence> [on <recipient>] with \"annotation\"";
    "prefix the specified messages with the given annotation.";
    set_task_perms(player);
    if (!(p = this:parse_mailread_cmd("@annotate", args, "cur", "on", 1)))
      "...lose...";
    elseif (length(p[4]) != 2 || p[4][1] != "with")
      player:notify(tostr("Usage:  ", verb, " [<message numbers>] [on <folder>] with <annotation>"));
    else
      {target, message_sequence, _, trailing_args} = p;
      annotation = trailing_args[2..$];
      annotation[1] = tostr("[", player.name, " (", player, "):  ", annotation[1], "]");
      if (typeof(e = target:annotate_message_seq(annotation, "prepend", message_sequence)) in {ERR, STR})
        player:notify(tostr("Annotation Failed:  ", e));
      else
        count = $seq_utils:size(message_sequence);
        player:notify(tostr("Annotating ", count, " message", count == 1 ? "" | "s", " on ", $mail_agent:name(target), " with:"));
        player:notify_lines(annotation);
      endif
    endif
    "Copied from annotatetest (#87053):@annotate [verb author Puff (#1449)] at Mon Feb 14 14:45:41 2005 PST";
  endverb

  verb annotate_message_seq (this none this) owner: #2 flags: "rxd"
    return "Cannot annotate player messages.";
  endverb

  verb check_mail (this none this) owner: #2 flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      nm = this:length_all_msgs() - this:length_date_le(this:get_current_message()[2]);
      if (nm)
        this:notify(tostr("You have new mail (", nm, " message", nm == 1 ? "" | "s", ").", this:mail_option("expert") ? "" | "  Type 'help mail' for info on reading it."));
      endif
    endif
  endverb
endobject