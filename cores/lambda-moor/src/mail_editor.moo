object MAIL_EDITOR
  name: "Mail Room"
  parent: GENERIC_EDITOR
  owner: HACKER
  readable: true

  property recipients (owner: HACKER, flags: "") = {};
  property replytos (owner: HACKER, flags: "") = {};
  property sending (owner: HACKER, flags: "") = {};
  property subjects (owner: HACKER, flags: "") = {};

  override aliases = {"Mail Room"};
  override blessed_task = 2043059065;
  override commands = {
    {"subj*ect", "[<text>]"},
    {"to", "[<rcpt>..]"},
    {"also-to", "[<rcpt>..]"},
    {"reply-to", "[<rcpt>..]"},
    {"who", "[<rcpt>..]"},
    {"pri*nt", ""},
    {"send", ""},
    {"showlists,unsubscribe", ""}
  };
  override commands2 = {
    {
      "say",
      "emote",
      "lis*t",
      "ins*ert",
      "n*ext,p*rev",
      "enter",
      "del*ete",
      "f*ind",
      "s*ubst",
      "m*ove,c*opy",
      "join*l",
      "fill"
    },
    {
      "y*ank",
      "w*hat",
      "subj*ect",
      "to",
      "also-to",
      "reply-to",
      "showlists,unsubscribe",
      "who",
      "pri*nt",
      "send",
      "abort",
      "q*uit,done,pause"
    }
  };
  override depart_msg = "%N flattens out into a largish postage stamp and floats away.";
  override entrances = {#16500};
  override exit_on_abort = 1;
  override help = {};
  override import_export_id = "mail_editor";
  override no_littering_msg = {
    "Saving your message so that you can finish it later.",
    "To come back, give the `@send' command with no arguments.",
    "Please come back and SEND or ABORT if you don't intend to be working on this",
    "message in the immediate future.  Keep Our MOO Clean!  No Littering!"
  };
  override no_text_msg = "Message body is empty.";
  override nothing_loaded_msg = "You're not editing anything!";
  override object_size = {22248, 1084848672};
  override previous_session_msg = "You need to either SEND it or ABORT it before you can start another message.";
  override return_msg = "A largish postage stamp floats into the room and fattens up into %n.";
  override stateprops = {
    {"sending", 0},
    {"replytos", {}},
    {"recipients", {}},
    {"subjects", ""},
    {"texts", {}},
    {"changes", 0},
    {"inserting", 1},
    {"readable", 0}
  };
  override who_location_msg = "%L [mailing]";

  verb working_on (this none this) owner: HACKER flags: "rxd"
    return this:ok(who = args[1]) && tostr("a letter ", this:sending(who) ? "(in transit) " | "", "to ", this:recipient_names(who), (subject = `this.subjects[who] ! ANY') && tostr(" entitled \"", subject, "\""));
  endverb

  verb parse_invoke (this none this) owner: HACKER flags: "rxd"
    "invoke(rcptstrings,verb[,subject]) for a @send";
    "invoke(1,verb,rcpts,subject,replyto,body) if no parsing is needed";
    "invoke(2,verb,msg,flags,replytos) for an @answer";
    if (!(which = args[1]))
      player:tell_lines({tostr("Usage:  ", args[2], " <list-of-recipients>"), tostr("        ", args[2], "                      to continue with a previous draft")});
    elseif (typeof(which) == TYPE_LIST)
      "...@send...";
      if (rcpts = this:parse_recipients({}, which))
        if (replyto = player:mail_option("replyto"))
          replyto = this:parse_recipients({}, replyto, ".mail_options: ");
        endif
        if (0 == (subject = {@args, 0}[3]))
          if (player:mail_option("nosubject"))
            subject = "";
          else
            player:tell("Subject:");
            subject = $command_utils:read();
          endif
        endif
        return {rcpts, subject, replyto, {}};
      endif
    elseif (which == 1)
      return args[3..6];
    elseif (!(to_subj = this:parse_msg_headers(msg = args[3], flags = args[4])))
    else
      include = {};
      if ("include" in flags)
        prefix = ">            ";
        for line in ($mail_agent:to_text(@msg))
          if (!line)
            prefix = ">  ";
            include = {@include, prefix};
          else
            include = {@include, @this:fill_string(">  " + line, 70, prefix)};
          endif
        endfor
      endif
      return {@to_subj, args[5], include};
    endif
    return 0;
  endverb

  verb init_session (this none this) owner: HACKER flags: "rxd"
    {who, recip, subj, replyto, msg} = args;
    if (this:ok(who))
      this.sending[who] = 0;
      this.recipients[who] = recip;
      this.subjects[who] = subj;
      this.replytos[who] = replyto || {};
      this:load(who, msg);
      this.active[who]:tell("Composing ", this:working_on(who));
      p = this.active[who];
      "if (p:mail_option(\"enter\") && !args[5])";
      "Changed from above so that @reply can take advantage of @mailoption +enter. Ho_Yan 11/9/94";
      if (p:mail_option("enter"))
        if (typeof(lines = $command_utils:read_lines()) == TYPE_ERR)
          p:tell(lines);
        else
          this:insert_line(p in this.active, lines, 0);
        endif
      endif
    endif
  endverb

  verb "pri*nt" (any none none) owner: HACKER flags: "rd"
    if (!dobjstr)
      plyr = player;
    elseif ($command_utils:player_match_result(plyr = $string_utils:match_player(dobjstr), dobjstr)[1])
      return;
    endif
    if (plyr != player && !this:readable(plyr in this.active))
      player:tell(plyr.name, "(", plyr, ") has not published anything here.");
    elseif (typeof(msg = this:message_with_headers(plyr in this.active)) != TYPE_LIST)
      player:tell(msg);
    else
      player:display_message({(plyr == player ? "Your" | tostr(plyr.name, "(", plyr, ")'s")) + " message so far:", ""}, player:msg_text(@msg));
    endif
  endverb

  verb message_with_headers (this none this) owner: HACKER flags: "rxd"
    return this:readable(who = args[1]) || this:ok(who) && $mail_agent:make_message(this.active[who], this.recipients[who], {this.subjects[who], this.replytos[who]}, this:text(who));
  endverb

  verb "subj*ect:" (any any any) owner: HACKER flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (!argstr)
      player:tell(this.subjects[who]);
    elseif (TYPE_ERR == typeof(subj = this:set_subject(who, argstr)))
      player:tell(subj);
    else
      player:tell(subj ? "Setting the subject line for your message to \"" + subj + "\"." | "Deleting the subject line for your message.");
    endif
  endverb

  verb set_subject (this none this) owner: HACKER flags: "rxd"
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    else
      this.subjects[who] = subj = args[2];
      this:set_changed(who, 1);
      return subj;
    endif
  endverb

  verb sending (this none this) owner: HACKER flags: "rxd"
    if (!(fuckup = this:ok(who = args[1])))
      return fuckup;
    elseif (!(task = this.sending[who]) || $code_utils:task_valid(task))
      return task;
    else
      "... uh oh... sending task crashed...";
      this:set_changed(who, 1);
      return this.sending[who] = 0;
    endif
  endverb

  verb "to*:" (any any any) owner: HACKER flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    elseif (!args)
      player:tell("Your message is currently to ", this:recipient_names(who), ".");
    else
      this.recipients[who] = this:parse_recipients({}, args);
      this:set_changed(who, 1);
      player:tell("Your message is now to ", this:recipient_names(who), ".");
    endif
  endverb

  verb "also*-to: cc*:" (any any any) owner: HACKER flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    else
      this.recipients[who] = this:parse_recipients(this.recipients[who], args);
      this:set_changed(who, 1);
      player:tell("Your message is now to ", this:recipient_names(who), ".");
    endif
  endverb

  verb "not-to*: uncc*:" (any any any) owner: HACKER flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    else
      recips = this.recipients[who];
      nonmrs = {};
      mrs = {};
      for o in (recips)
        if ($object_utils:isa(o, $mail_recipient))
          mrs = {@mrs, o};
        else
          nonmrs = {@nonmrs, o};
        endif
      endfor
      for a in (args)
        if (!a)
          player:tell("\"\"?");
          return;
        elseif (valid(aobj = $mail_agent:match_recipient(a)) && aobj in recips)
        elseif ($failed_match != (aobj = $string_utils:literal_object(a)))
          if (!(aobj in recips))
            player:tell(aobj, " was not a recipient.");
            return;
          endif
        elseif (a[1] == "*" && valid(aobj = $string_utils:match(a[2..$], mrs, "aliases")))
        elseif (a[1] != "*" && valid(aobj = $string_utils:match(a, nonmrs, "aliases")))
        elseif (valid(aobj = $string_utils:match(a, recips, "aliases")))
        else
          player:tell("couldn't find \"", a, "\" in To: list.");
          return;
        endif
        recips = setremove(recips, aobj);
      endfor
      this.recipients[who] = recips;
      this:set_changed(who, 1);
      player:tell("Your message is now to ", this:recipient_names(who), ".");
    endif
  endverb

  verb parse_recipients (this none this) owner: HACKER flags: "rxd"
    "parse_recipients(prev_list,list_of_strings) -- parses list of strings and adds any resulting player objects to prev_list.  Optional 3rd arg is prefixed to any mismatch error messages";
    {recips, l, ?cmd_id = ""} = args;
    cmd_id = cmd_id || "";
    for s in (typeof(l) == TYPE_LIST ? l | {l})
      if (typeof(s) != TYPE_STR)
        if ($mail_agent:is_recipient(s))
          recips = setadd(recips, s);
        else
          player:tell(cmd_id, s, " is not a valid mail recipient.");
        endif
      elseif (!$mail_agent:match_failed(md = $mail_agent:match_recipient(s), s, cmd_id))
        recips = setadd(recips, md);
      endif
    endfor
    return recips;
  endverb

  verb recipient_names (this none this) owner: HACKER flags: "rxd"
    return this:ok(who = args[1]) && $mail_agent:name_list(@this.recipients[who]);
  endverb

  verb make_message (this none this) owner: HACKER flags: "rxd"
    return $mail_agent:make_message(@args);
  endverb

  verb name_list (this none this) owner: HACKER flags: "rxd"
    "(obsolete verb... see $mail_agent:name_list)";
    return $mail_agent:(verb)(@args[1]);
  endverb

  verb parse_msg_headers (this none this) owner: HACKER flags: "rxd"
    "parse_msg_headers(msg,flags)";
    "  parses msg to extract reply recipients and construct a subject line";
    "  if the \"all\" flag is present, reply goes to all of the original recipients";
    "  returns a list {recipients, subjectline} or 0 in case of error.";
    {msg, flags} = args;
    replyall = "all" in flags;
    objects = {};
    if ("followup" in flags)
      "...look for first non-player recipient in To: line...";
      for o in ($mail_agent:parse_address_field(msg[3]))
        if (objects)
          break o;
        elseif ($object_utils:isa(o, $mail_recipient))
          objects = {o};
        endif
      endfor
    endif
    objects = objects || $mail_agent:parse_address_field(msg[2] + (replyall ? msg[3] | ""));
    for line in (msg[5..("" in {@msg, ""}) - 1])
      if (rt = index(line, "Reply-to:") == 1)
        objects = $mail_agent:parse_address_field(line);
      endif
    endfor
    recips = {};
    for o in (objects)
      if (o == #0)
        player:tell("Sorry, but I can't parse the header of that message.");
        return 0;
      elseif (!valid(o) || !(is_player(o) || $mail_recipient in $object_utils:ancestors(o)))
        player:tell(o, " is no longer a valid player or maildrop; ignoring that recipient.");
      elseif (o != player)
        recips = setadd(recips, o);
      endif
    endfor
    subject = msg[4];
    if (subject == " ")
      subject = "";
    elseif (subject && index(subject, "Re: ") != 1)
      subject = "Re: " + subject;
    endif
    return {recips, subject};
  endverb

  verb check_answer_flags (this none this) owner: HACKER flags: "rxd"
    flags = {};
    for o in ({"all", "include", "followup"})
      if (player:mail_option(o))
        flags = {@flags, o};
      endif
    endfor
    reply_to = player:mail_option("replyto") || {};
    flaglist = "+1#include -1#noinclude +2#all -2#sender 0#replyto +3#followup ";
    for a in (args)
      if (i = index(a, "="))
        value = a[i + 1..$];
        a = a[1..i - 1];
      else
        value = "";
      endif
      if (typeof(a) != TYPE_STR || (i = index(flaglist, "#" + a)) < 3)
        player:tell("Unrecognized answer/reply option:  ", a);
        return 0;
      elseif (i != rindex(flaglist, "#" + a))
        player:tell("Ambiguous answer/reply option:  ", a);
        return 0;
      elseif (j = index("0123456789", flaglist[i - 1]) - 1)
        if (value)
          player:tell("Flag does not take a value:  ", a);
          return 0;
        endif
        f = {"include", "all", "followup"}[j];
        flags = flaglist[i - 2] == "+" ? setadd(flags, f) | setremove(flags, f);
        if (f == "all")
          flags = setremove(flags, "followup");
        endif
      elseif (!value || (value = this:parse_recipients({}, $string_utils:explode(value), "replyto flag:  ")))
        reply_to = value || {};
      endif
    endfor
    return {flags, reply_to};
  endverb

  verb "reply-to*: replyto*:" (any any any) owner: HACKER flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
    else
      if (args)
        this.replytos[who] = rt = this:parse_recipients({}, args);
        this:set_changed(who, 1);
      else
        rt = this.replytos[who];
      endif
      player:tell(rt ? "Replies will go to " + $mail_agent:name_list(@this.replytos[who]) + "." | "Reply-to field is empty.");
    endif
  endverb

  verb send (none none none) owner: #2 flags: "rd"
    "WIZARDLY";
    if (!(who = this:loaded(player)))
      player:notify(this:nothing_loaded_msg());
    elseif (!(recips = this.recipients[who]))
      player:notify("Umm... your message isn't addressed to anyone.");
    elseif (this:sending(who))
      player:notify("Again? ... relax... it'll get there eventually.");
    else
      msg = this:message_with_headers(who);
      this.sending[who] = old_sending = task_id();
      this:set_changed(who, 0);
      player:notify("Sending...");
      "... this sucker can suspend BIG TIME...";
      result = $mail_agent:raw_send(msg, recips, player);
      "... the world changes...";
      who = player in this.active;
      if (who && this.sending[who] == old_sending)
        "... same editing session; no problemo...";
        previous = "";
        this.sending[who] = 0;
      else
        "... uh oh, different session... tiptoe quietly out...";
        "... Don't mess with the session.";
        previous = "(prior send) ";
      endif
      if (!(e = result[1]))
        player:notify(tostr(previous, typeof(e) == TYPE_ERR ? e | "Bogus recipients:  " + $string_utils:from_list(result[2])));
        player:notify(tostr(previous, "Mail not sent."));
        previous || this:set_changed(who, 1);
      elseif (length(result) == 1)
        player:notify(tostr(previous, "Mail not actually sent to anyone."));
        previous || this:set_changed(who, 1);
      else
        player:notify(tostr(previous, "Mail actually sent to ", $mail_agent:name_list(@listdelete(result, 1))));
        if (previous)
          "...don't even think about it...";
        elseif (player.location == this)
          if (ticks_left() < 10000)
            suspend(0);
          endif
          this:done();
        elseif (!this:changed(who))
          "... player is gone, no further edits...";
          this:kill_session(who);
        endif
      endif
    endif
  endverb

  verb who (any none none) owner: HACKER flags: "rxd"
    if (dobjstr)
      if (!(recips = this:parse_recipients({}, args)))
        "parse_recipients has already complained about anything it doesn't like";
        return;
      endif
    elseif (caller != player)
      return E_PERM;
    elseif (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    else
      recips = this.recipients[who];
    endif
    resolve = $mail_agent:resolve_addr(recips, player);
    if (resolve[1])
      player:tell("Bogus addresses:  ", $string_utils:english_list(resolve[1]));
    else
      player:tell(dobjstr ? "Mail to " + $mail_agent:name_list(@recips) + " actually goes to " | "Your mail will actually go to ", $mail_agent:name_list(@resolve[2]));
    endif
  endverb

  verb showlists (any none none) owner: HACKER flags: "rd"
    player:tell_lines({"Available aliases:", ""});
    for c in (dobjstr == "all" ? $object_utils:descendants($mail_recipient) | $mail_agent.contents)
      if (c:is_usable_by(player) || c:is_readable_by(player))
        c:look_self();
      endif
    endfor
  endverb

  verb "subsc*ribe" (any at any) owner: HACKER flags: "rd"
    player:tell("This command is obsolete.  Use @subscribe instead.  See `help @subscribe'");
    return;
    if (!iobjstr)
      player:tell("Usage:  ", verb, " [<list-of-people/lists>] to <list>");
      return;
    elseif ($mail_agent:match_failed(iobj = $mail_agent:match(iobjstr), iobjstr))
      return;
    endif
    rstrs = dobjstr ? $string_utils:explode(dobjstr) | {"me"};
    recips = this:parse_recipients({}, rstrs);
    outcomes = iobj:add_forward(@recips);
    if (typeof(outcomes) != TYPE_LIST)
      player:tell(outcomes);
      return;
    endif
    added = {};
    for r in [1..length(recips)]
      if ((t = typeof(e = outcomes[r])) == TYPE_OBJ)
        added = setadd(added, recips[r]);
      else
        player:tell(verb, " ", recips[r].name, " to ", iobj.name, ":  ", e);
      endif
    endfor
    if (added)
      player:tell($string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%(name) (%#)", added)), " added to ", iobj.name, " (", iobj, ")");
    endif
  endverb

  verb "unsubsc*ribe" (any from any) owner: HACKER flags: "rd"
    if (!iobjstr)
      player:tell("Usage:  ", verb, " [<list-of-people/lists>] from <list>");
      return;
    elseif ($mail_agent:match_failed(iobj = $mail_agent:match(iobjstr), iobjstr))
      return;
    endif
    rstrs = dobjstr ? $string_utils:explode(dobjstr) | {"me"};
    recips = this:parse_recipients({}, rstrs);
    outcomes = iobj:delete_forward(@recips);
    if (typeof(outcomes) != TYPE_LIST)
      player:tell(outcomes);
      return;
    endif
    removed = {};
    for r in [1..length(recips)]
      if (typeof(e = outcomes[r]) == TYPE_ERR)
        player:tell(verb, " ", recips[r].name, " from ", iobj.name, ":  ", e == E_INVARG ? "Not on list." | e);
      else
        removed = setadd(removed, recips[r]);
      endif
    endfor
    if (removed)
      player:tell($string_utils:english_list($list_utils:map_arg(2, $string_utils, "pronoun_sub", "%(name) (%#)", removed)), " removed from ", iobj.name, " (", iobj, ")");
    endif
  endverb

  verb retain_session_on_exit (this none this) owner: HACKER flags: "rxd"
    return this:ok(who = args[1]) && (this:sending(who) || pass(@args));
  endverb

  verb no_littering_msg (this none this) owner: HACKER flags: "rxd"
    "recall that this only gets called if :retain_session_on_exit returns true";
    return this:ok(who = player in this.active) && !this:changed(who) ? {"Your message is in transit."} | this.(verb);
  endverb

  verb local_editing_info (this none this) owner: HACKER flags: "rxd"
    lines = {"To:       " + (toline = $mail_agent:name_list(@args[1])), "Subject:  " + $string_utils:trim(subject = args[2])};
    if (args[3])
      lines = {@lines, "Reply-to: " + $mail_agent:name_list(@args[3])};
    endif
    lines = {@lines, "", @args[4]};
    return {tostr("MOOMail", subject ? "(" + subject + ")" | "-to(" + toline + ")"), lines, "@@sendmail"};
  endverb
endobject