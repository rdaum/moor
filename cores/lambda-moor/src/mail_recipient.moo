object MAIL_RECIPIENT
  name: "Generic Mail Recipient"
  parent: ROOT_CLASS
  owner: HACKER
  fertile: true
  readable: true

  property email_validated (owner: HACKER, flags: "") = 1;
  property expire_period (owner: HACKER, flags: "r") = 2592000;
  property guests_can_send_here (owner: HACKER, flags: "rc") = 0;
  property last_msg_date (owner: HACKER, flags: "r") = 0;
  property last_used_time (owner: HACKER, flags: "r") = 0;
  property mail_forward (owner: HACKER, flags: "r") = "%t (%[#t]) is a generic recipient.";
  property mail_notify (owner: HACKER, flags: "r") = {};
  property messages (owner: HACKER, flags: "") = {};
  property messages_going (owner: HACKER, flags: "") = {};
  property messages_kept (owner: HACKER, flags: "r") = {};
  property moderated (owner: HACKER, flags: "rc") = {};
  property moderator_forward (owner: HACKER, flags: "rc") = "%n (%#) can't send to moderated list %t (%[#t]) directly.";
  property moderator_notify (owner: HACKER, flags: "rc") = {};
  property readers (owner: HACKER, flags: "rc") = {};
  property registered_email (owner: HACKER, flags: "") = "";
  property rmm_own_msgs (owner: HACKER, flags: "rc") = 1;
  property validation_password (owner: HACKER, flags: "") = "";
  property writers (owner: HACKER, flags: "rc") = {};

  override aliases = {"Generic Mail Recipient"};
  override description = "This can either be a mailing list or a mail folder, depending on what mood you're in...";
  override object_size = {30900, 1084848672};

  verb set_aliases (this none this) owner: HACKER flags: "rxd"
    "For changing mailing list aliases, we check to make sure that none of the aliases match existing mailing list aliases.  Aliases containing spaces are not used in addresses and so are not subject to this restriction ($mail_agent:match will not match on them, however, so they only match if used in the immediate room, e.g., with match_object() or somesuch).";
    "  => E_PERM   if you don't own this";
    {newaliases} = args;
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (this.location != $mail_agent)
      "... we don't care...";
      return pass(@args);
    elseif (length(newaliases) > $mail_agent.max_list_aliases)
      return E_QUOTA;
    else
      for a in (aliases = newaliases)
        if (index(a, " "))
          "... we don't care...";
        elseif (rp = $mail_agent:reserved_pattern(a))
          player:tell("Mailing list name \"", a, "\" uses a reserved pattern: ", rp[1]);
          aliases = setremove(aliases, a);
        elseif (valid(p = $mail_agent:match(a, #-1)) && (p != this && a in p.aliases))
          player:tell("Mailing list name \"", a, "\" in use on ", p.name, "(", p, ")");
          aliases = setremove(aliases, a);
        endif
      endfor
      return pass(aliases) && newaliases == aliases;
    endif
  endverb

  verb look_self (this none this) owner: HACKER flags: "rxd"
    "Returns full name and mail aliases for this list, read and write status by the player, and a short description. Calling :look_self(1) will omit the description.";
    {?brief = 0} = args;
    namelist = "*" + ((names = this:mail_names()) ? $string_utils:from_list(names, ", *") | tostr(this));
    if (typeof(fwd = this:mail_forward()) != LIST)
      fwd = {};
    endif
    if (this:is_writable_by(player))
      if (player in fwd)
        read = " [Writable/Subscribed]";
      else
        read = " [Writable]";
      endif
    elseif (this.readers == 1)
      read = tostr(" [Public", player in fwd ? "/Subscribed]" | "]");
    elseif (player in fwd)
      read = " [Subscribed]";
    elseif (this:is_readable_by(player))
      read = " [Readable]";
    else
      read = "";
    endif
    if (this:is_usable_by($no_one))
      mod = "";
    elseif (this:is_usable_by(player))
      mod = " [Approved]";
    else
      mod = " [Moderated]";
    endif
    player:tell(namelist, "  (", this, ")", read, mod);
    if (!brief)
      d = this:description();
      if (typeof(d) == STR)
        d = {d};
      endif
      for l in (d)
        if (length(l) <= 75)
          ls = {l};
        else
          ls = $generic_editor:fill_string(l, 76);
        endif
        for line in (ls)
          player:tell("    ", line);
          $command_utils:suspend_if_needed(0);
        endfor
      endfor
    endif
  endverb

  verb "is_writable_by is_annotatable_by" (this none this) owner: HACKER flags: "rxd"
    return $perm_utils:controls(who = args[1], this) || `who in this.writers ! E_TYPE';
  endverb

  verb is_readable_by (this none this) owner: HACKER flags: "rxd"
    return typeof(this.readers) != LIST || ((who = args[1]) in this.readers || (this:is_writable_by(who) || $mail_agent:sends_to(1, this, who)));
  endverb

  verb is_usable_by (this none this) owner: HACKER flags: "rxd"
    who = args[1];
    if (this.moderated)
      return `who in this.moderated ! E_TYPE' || (this:is_writable_by(who) || who.wizard);
    else
      return this.guests_can_send_here || !$object_utils:isa(who, $guest);
    endif
  endverb

  verb mail_notify (this none this) owner: HACKER flags: "rxd"
    if (args && !this:is_usable_by(args[1]) && !args[1].wizard)
      return this:moderator_notify(@args);
    else
      return this.(verb);
    endif
  endverb

  verb mail_forward (this none this) owner: HACKER flags: "rxd"
    if (args && !this:is_usable_by(args[1]) && !args[1].wizard)
      return this:moderator_forward(@args);
    elseif (typeof(mf = this.(verb)) == STR)
      return $string_utils:pronoun_sub(mf, @args);
    else
      return mf;
    endif
  endverb

  verb moderator_forward (this none this) owner: HACKER flags: "rxd"
    if (typeof(mf = this.(verb)) == STR)
      return $string_utils:pronoun_sub(mf, args ? args[1] | $player);
    else
      return mf;
    endif
  endverb

  verb add_forward (this none this) owner: HACKER flags: "rxd"
    ":add_forward(recip[,recip...]) adds new recipients to this list.  Returns a string error message or a list of results (recip => success, E_PERM => not allowed, E_INVARG => not a valid recipient, string => other kind of failure)";
    if (caller == $mail_editor)
      perms = player;
    else
      perms = caller_perms();
    endif
    result = {};
    forward_self = !this.mail_forward || this in this.mail_forward;
    for recip in (args)
      if (!valid(recip) || (!is_player(recip) && !($mail_recipient in $object_utils:ancestors(recip))))
        r = E_INVARG;
      elseif ($perm_utils:controls(perms, this) || (typeof(this.readers) != LIST && $perm_utils:controls(perms, recip)))
        this.mail_forward = setadd(this.mail_forward, recip);
        r = recip;
      else
        r = E_PERM;
      endif
      result = listappend(result, r);
    endfor
    if (length(this.mail_forward) > 1 && $nothing in this.mail_forward)
      this.mail_forward = setremove(this.mail_forward, $nothing);
    endif
    if (forward_self)
      this.mail_forward = setadd(this.mail_forward, this);
    endif
    return result;
  endverb

  verb delete_forward (this none this) owner: HACKER flags: "rxd"
    ":delete_forward(recip[,recip...]) removes recipients to this list.  Returns a list of results (E_PERM => not allowed, E_INVARG => not on list)";
    if (caller == $mail_editor)
      perms = player;
    else
      perms = caller_perms();
    endif
    result = {};
    forward_self = !this.mail_forward || this in this.mail_forward;
    for recip in (args)
      if (!(recip in this.mail_forward))
        r = E_INVARG;
      elseif (!valid(recip) || $perm_utils:controls(perms, recip) || $perm_utils:controls(perms, this))
        if (recip == this)
          forward_self = 0;
        endif
        this.mail_forward = setremove(this.mail_forward, recip);
        r = recip;
      else
        r = E_PERM;
      endif
      result = listappend(result, r);
    endfor
    if (!(forward_self || this.mail_forward))
      this.mail_forward = {$nothing};
    elseif (this.mail_forward == {this})
      this.mail_forward = {};
    endif
    return result;
  endverb

  verb add_notify (this none this) owner: HACKER flags: "rxd"
    ":add_notify(recip[,recip...]) adds new notifiees to this list.  Returns a list of results (recip => success, E_PERM => not allowed, E_INVARG => not a valid recipient)";
    if (caller == $mail_editor)
      perms = player;
    else
      perms = caller_perms();
    endif
    result = {};
    for recip in (args)
      if (!valid(recip) || recip == this)
        r = E_INVARG;
      elseif ($perm_utils:controls(perms, this) || (this:is_readable_by(perms) && $perm_utils:controls(perms, recip)))
        this.mail_notify = setadd(this.mail_notify, recip);
        r = recip;
      else
        r = E_PERM;
      endif
      result = listappend(result, r);
    endfor
    return result;
  endverb

  verb delete_notify (this none this) owner: HACKER flags: "rxd"
    ":delete_notify(recip[,recip...]) removes notifiees from this list.  Returns a list of results (E_PERM => not allowed, E_INVARG => not on list)";
    if (caller == $mail_editor)
      perms = player;
    else
      perms = caller_perms();
    endif
    result = {};
    rmthis = 0;
    for recip in (args)
      if (!(recip in this.mail_notify))
        r = E_INVARG;
      elseif (!valid(recip) || ($perm_utils:controls(perms, recip) || $perm_utils:controls(perms, this)))
        if (recip == this)
          rmthis = 1;
        endif
        this.mail_notify = setremove(this.mail_notify, recip);
        r = recip;
      else
        r = E_PERM;
      endif
      result = listappend(result, r);
    endfor
    return result;
  endverb

  verb receive_message (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    else
      this.messages = {@this.messages, {new = this:new_message_num(), args[1]}};
      this.last_msg_date = args[1][1];
      this.last_used_time = time();
      return new;
    endif
  endverb

  verb ok (this none this) owner: HACKER flags: "rxd"
    ":ok(caller,callerperms) => true iff caller can do read operations";
    return args[1] in {this, $mail_agent} || (args[2].wizard || this:is_readable_by(args[2]));
  endverb

  verb ok_write (this none this) owner: HACKER flags: "rxd"
    ":ok_write(caller,callerperms) => true iff caller can do write operations";
    return args[1] in {this, $mail_agent} || (args[2].wizard || this:is_writable_by(args[2]));
  endverb

  verb "parse_message_seq from_msg_seq %from_msg_seq to_msg_seq %to_msg_seq subject_msg_seq body_msg_seq kept_msg_seq unkept_msg_seq display_seq_headers display_seq_full messages_in_seq list_rmm new_message_num length_num_le length_date_le length_all_msgs exists_num_eq msg_seq_to_msg_num_list msg_seq_to_msg_num_string" (this none this) owner: HACKER flags: "rxd"
    ":parse_message_seq(strings,cur) => {msg_seq,@unused_strings} or string error";
    "";
    ":from_msg_seq(olist)     => msg_seq of messages from those people";
    ":%from_msg_seq(strings)  => msg_seq of messages with strings in the From: line";
    ":to_msg_seq(olist)       => msg_seq of messages to those people";
    ":%to_msg_seq(strings)    => msg_seq of messages with strings in the To: line";
    ":subject_msg_seq(target) => msg_seq of messages with target in the Subject:";
    ":body_msg_seq(target)    => msg_seq of messages with target in the body";
    ":new_message_num()    => number that the next incoming message will receive.";
    ":length_num_le(num)   => number of messages in folder numbered <= num";
    ":length_date_le(date) => number of messages in folder dated <= date";
    ":length_all_msgs()    => number of messages in folder";
    ":exists_num_eq(num)   => index of message in folder numbered == num, or 0";
    "";
    ":display_seq_headers(msg_seq[,cur])   display message summary lines";
    ":display_seq_full(msg_seq[,preamble]) display entire messages";
    "            => number of final message displayed";
    ":list_rmm() displays contents of .messages_going.";
    "            => the number of messages in .messages_going.";
    "";
    ":messages_in_seq(msg_seq) => list of messages in msg_seq on folder";
    "";
    "See the corresponding routines on $mail_agent for more detail.";
    return this:ok(caller, caller_perms()) ? $mail_agent:(verb)(@args) | E_PERM;
  endverb

  verb length_date_gt (this none this) owner: HACKER flags: "rxd"
    ":length_date_le(date) => number of messages in folder dated > date";
    "";
    if (this:ok(caller, caller_perms()))
      date = args[1];
      return this.last_msg_date <= date ? 0 | $mail_agent:(verb)(date);
    else
      return E_PERM;
    endif
  endverb

  verb rm_message_seq (this none this) owner: HACKER flags: "rxd"
    ":rm_message_seq(msg_seq) removes the given sequence of from folder";
    "               => string giving msg numbers removed";
    "See the corresponding routine on $mail_agent.";
    if (this:ok_write(caller, caller_perms()))
      return $mail_agent:(verb)(@args);
    elseif (this:ok(caller, caller_perms()) && (seq = this:own_messages_filter(caller_perms(), @args)))
      return $mail_agent:(verb)(@listset(args, seq, 1));
    else
      return E_PERM;
    endif
  endverb

  verb "undo_rmm expunge_rmm renumber keep_message_seq set_message_body_by_index" (this none this) owner: HACKER flags: "rxd"
    ":rm_message_seq(msg_seq) removes the given sequence of from folder";
    "               => string giving msg numbers removed";
    ":list_rmm()    displays contents of .messages_going.";
    "               => number of messages in .messages_going.";
    ":undo_rmm()    restores previously deleted messages from .messages_going.";
    "               => msg_seq of restored messages";
    ":expunge_rmm() destroys contents of .messages_going once and for all.";
    "               => number of messages in .messages_going.";
    ":renumber([cur])  renumbers all messages";
    "               => {number of messages,new cur}.";
    ":set_message_body_by_index(i,newbody)";
    "               changes the body of the i-th message.";
    "";
    "See the corresponding routines on $mail_agent.";
    return this:ok_write(caller, caller_perms()) ? $mail_agent:(verb)(@args) | E_PERM;
  endverb

  verb own_messages_filter (this none this) owner: HACKER flags: "rxd"
    ":own_messages_filter(who,msg_seq) => subsequence of msg_seq consisting of those messages that <who> is actually allowed to remove (on the assumption that <who> is not one of the allowed writers of this folder.";
    if (!this.rmm_own_msgs)
      return E_PERM;
    elseif (typeof(seq = this:from_msg_seq({args[1]}, args[2])) != LIST || seq != args[2])
      return {};
    else
      return seq;
    endif
  endverb

  verb messages (this none this) owner: HACKER flags: "rxd"
    "NOTE:  this routine is obsolete, use :messages_in_seq()";
    ":messages(num) => returns the message numbered num.";
    ":messages()    => returns the entire list of messages (can be SLOW).";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    elseif (!args)
      return this:messages_in_seq({1, this:length_all_msgs() + 1});
    elseif (!(n = this:exists_num_eq(args[1])))
      return E_RANGE;
    else
      return this:messages_in_seq(n)[2];
    endif
  endverb

  verb date_sort (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    date_seq = {};
    for msg in (this.messages)
      date_seq = {@date_seq, msg[2][1]};
    endfor
    msg_order = $list_utils:sort($list_utils:range(n = length(msgs = this.messages)), date_seq);
    newmsgs = {};
    for i in [1..n]
      if ($command_utils:suspend_if_needed(0))
        player:tell("...", i);
      endif
      newmsgs = {@newmsgs, {i, msgs[msg_order[i]][2]}};
    endfor
    if (length(this.messages) != n)
      "...shit, new mail received,... start again...";
      fork (0)
        this:date_sort();
      endfork
    else
      this.messages = newmsgs;
      this.last_used_time = newmsgs[$][2][1];
    endif
  endverb

  verb _fix_last_msg_date (this none this) owner: HACKER flags: "rxd"
    mlen = this:length_all_msgs();
    this.last_msg_date = mlen && this:messages_in_seq(mlen)[2][1];
  endverb

  verb moderator_notify (this none this) owner: HACKER flags: "rxd"
    return this.(verb);
  endverb

  verb msg_summary_line (this none this) owner: HACKER flags: "rxd"
    return $mail_agent:msg_summary_line(@args);
  endverb

  verb __check (this none this) owner: HACKER flags: "rxd"
    for m in (this.messages)
      $mail_agent:__convert_new(@m[2]);
      $command_utils:suspend_if_needed(0);
    endfor
  endverb

  verb __fix (this none this) owner: #2 flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    msgs = {};
    i = 1;
    for m in (oldmsgs = this.messages)
      msgs = {@msgs, {m[1], $mail_agent:__convert_new(@m[2])}};
      if ($command_utils:running_out_of_time())
        player:notify(tostr("...", i, " ", this));
        suspend(0);
        if (oldmsgs != this.messages)
          return 0;
        endif
      endif
      i = i + 1;
    endfor
    this.messages = msgs;
    return 1;
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      if (!(this in {$mail_recipient, $big_mail_recipient}))
        "...generic mail recipients stay in #-1...";
        move(this, $mail_agent);
        this:rm_message_seq($seq_utils:range(1, this:length_all_msgs()));
        this:expunge_rmm();
        this:_fix_last_msg_date();
        this.mail_forward = {};
        for p in ({"mail_notify", "moderator_forward", "moderator_notify", "writers", "readers", "expire_period", "last_used_time"})
          this.(p) = $mail_recipient.(p);
        endfor
      endif
    endif
  endverb

  verb initialize (this none this) owner: #2 flags: "rxd"
    if (caller == this || $perm_utils:controls(caller_perms(), this))
      this.mail_forward = {};
      return pass(@args);
    endif
  endverb

  verb "mail_name_old mail_name short_mail_name" (this none this) owner: HACKER flags: "rxd"
    return "*" + this.aliases[1];
  endverb

  verb mail_names (this none this) owner: HACKER flags: "rxd"
    names = {};
    for a in (this.aliases)
      if (!index(a, " "))
        names = setadd(names, strsub(a, "_", "-"));
      endif
    endfor
    return names;
  endverb

  verb expire_old_messages (this none this) owner: #2 flags: "rxd"
    if (this:ok_write(caller, caller_perms()))
      if ($network.active)
        "Passed security check...";
        set_task_perms($wiz_utils:random_wizard());
        for x in (this.mail_notify)
          if (!$object_utils:has_verb(x, "notify_mail"))
            "In theory I should call this:delete_notify but it's ugly and ticky as sin and I'm lazy.";
            this.mail_notify = setremove(this.mail_notify, x);
          endif
        endfor
        if (this.expire_period && (rmseq = $seq_utils:remove(this:unkept_msg_seq(), 1 + this:length_date_le(time() - this.expire_period))))
          "... i.e., everything not marked kept that is older than expire_period";
          if (this.registered_email && this.email_validated)
            format = this.owner:format_for_netforward(this:messages_in_seq(rmseq), " expired from " + $mail_agent:name(this));
            $network:sendmail(this.registered_email, @{format[2], @format[1]});
            "Do nothing if it bounces, etc.";
          endif
          this:rm_message_seq(rmseq);
          return this:expunge_rmm();
        else
          return 0;
        endif
      endif
    else
      return E_PERM;
    endif
  endverb

  verb moveto (this none this) owner: HACKER flags: "rxd"
    if (this:is_writable_by(caller_perms()) || this:is_writable_by(caller))
      pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb msg_full_text (this none this) owner: HACKER flags: "rxd"
    ":msg_full_text(@msg) => list of strings.";
    "msg is a mail message (in the usual transmission format).";
    "display_seq_full calls this to obtain the actual list of strings to display.";
    return player:msg_text(@args);
    "default is to leave it up to the player how s/he wants it to be displayed.";
  endverb

  verb "@set_expire" (this at any) owner: HACKER flags: "rxd"
    "Syntax:  @set_expire <recipient> to <time>";
    "         @set_expire <recipient> to";
    "";
    "Allows the list owner to set the expiration period of this mail recipient. This is the time messages will remain before they are removed from the list. The <time> given can be in english terms (e.g., 2 months, 45 days, etc.).";
    "Non-wizard mailing list owners are limited to a maximum expire period of 180 days. They are also prohibited from setting the list to non-expiring.";
    "Wizards may set the expire period to 0 for no expiration.";
    "The second form, leaving off the time specification, will tell you what the recipient's expire period is currently set to.";
    if (caller_perms() != #-1 && caller_perms() != player)
      return player:tell(E_PERM);
    elseif (!this:is_writable_by(player))
      return player:tell(E_PERM);
    elseif (!iobjstr)
      return player:tell(this.expire_period ? tostr("Messages will automatically expire from ", this:mail_name(), " after ", $time_utils:english_time(this.expire_period), ".") | tostr("Messages will not expire from ", this:mail_name()));
    elseif (typeof(time = $time_utils:parse_english_time_interval(iobjstr)) == ERR)
      return player:tell(time);
    elseif (time == 0 && !player.wizard)
      return player:tell("Only wizards may set a mailing list to not expire.");
    elseif (time > 180 * 86400 && !player.wizard)
      return player:tell("Only a wizard may set the expiration period on a mailing list to greater than 180 days.");
    endif
    this.expire_period = time;
    player:tell("Messages will ", time != 0 ? tostr("automatically expire from ", this:mail_name(), " after ", $time_utils:english_time(time)) | tostr("not expire from ", this:mail_name()), ".");
  endverb

  verb "@register @netregister" (this at any) owner: #2 flags: "rxd"
    "Syntax:   @register <recipient> to <email-address>";
    "alias     @netregister <recipient> to <email-address>";
    "          @register <recipient> to";
    "";
    "The list owner may use this command to set a registered email address for the mail recipient. When set, mail messages that expire off of the mail recipient will be mailed to that address.";
    "If you leave the email address off of the command, it will return the current registration and expiration information for that recipient if you own it.";
    "The owner may register a mail recipient to any email address. However, if the address does not match his registered email address, then a password will be generated and sent to the address specified when this command is used. Then, the owner may retrieve that password and verify the address with the command:";
    "";
    "  @validate <recipient> with <password>";
    "";
    "See *B:MailingListReform #98087 for full details.";
    if (caller_perms() != #-1 && caller_perms() != player)
      return player:tell(E_PERM);
    elseif (!$perm_utils:controls(player, this))
      return player:tell(E_PERM);
    elseif (!iobjstr)
      if (this.registered_email)
        player:tell(this:mail_name(), " is registered to ", this.registered_email, ". Messages will be sent there when they expire after ", this.expire_period == 0 ? "never" | $time_utils:english_time(this.expire_period), ".");
      else
        player:tell(this:mail_name(), " is not registered to any address. Messages will be deleted when they expire after ", this.expire_period == 0 ? "never" | $time_utils:english_time(this.expire_period), ".");
        player:tell("Usage:  @register <recipient> to <email-address>");
      endif
      return;
    elseif (iobjstr == $wiz_utils:get_email_address(player))
      this.registered_email = $wiz_utils:get_email_address(player);
      this.email_validated = 1;
      player:tell("Messages expired from ", this:mail_name(), " after ", this.expire_period == 0 ? "never" | $time_utils:english_time(this.expire_period), " will be emailed to ", this.registered_email, " (which is your registered email address).");
    elseif (reason = $network:invalid_email_address(iobjstr))
      return player:tell(reason, ".");
    elseif (!$network.active)
      return player:tell("The network is not up at the moment. Please try again later or contact a wizard for help.");
    else
      password = $wiz_utils:random_password(5);
      result = $network:sendmail(iobjstr, tostr($network.MOO_Name, " mailing list verification"), @$generic_editor:fill_string(tostr("The mailing list ", this:mail_name(), " on ", $network.MOO_Name, " has had this address designated as the recipient of expired mail messages. If this is not correct, then you need do nothing but ignore this message. If this is correct, you must log into the MOO and type:  `@validate ", this:mail_name(), " with ", password, "' to start receiving expired mail messages."), 75));
      if (result != 0)
        return player:tell("Mail sending did not work: ", result, ". Address not set.");
      endif
      this.registered_email = iobjstr;
      this.email_validated = 0;
      this.validation_password = password;
      player:tell("Registration complete. Password sent to the address you specified. When you receive the email, log back in to validate it with the command:  @validate <recipient> with <password>. If you do not receive the password email, try again or notify a wizard if this is a recurring problem.");
    endif
  endverb

  verb "@validate" (this with any) owner: HACKER flags: "rxd"
    "Syntax:  @validate <recipient> with <password>";
    "";
    "This command is used to validate an email address set to receive expired messages that did not match the list owner's registered email address. When using the @register command, a password was sent via email to the address specified. This command is to verify that the password was received properly.";
    if (caller_perms() != #-1 && caller_perms() != player)
      return player:tell(E_PERM);
    elseif (!$perm_utils:controls(player, this))
      return player:tell(E_PERM);
    elseif (!this.registered_email)
      return player:tell("No email address has even been set for ", this:mail_name(), ".");
    elseif (this.email_validated)
      return player:tell("The email address for ", this:mail_name(), " has already been validated.");
    elseif (!iobjstr)
      return player:tell("Usage:  @validate <recipient> with <password>");
    elseif (iobjstr != this.validation_password)
      return player:tell("That is not the correct password.");
    else
      this.email_validated = 1;
      player:tell("Password validated. Messages that expire after ", this.expire_period == 0 ? "never" | $time_utils:english_time(this.expire_period), " from ", this:mail_name(), " will be emailed to ", this.registered_email, ".");
    endif
  endverb

  verb set_name (this none this) owner: HACKER flags: "rxd"
    {name} = args;
    if (caller != this && !$perm_utils:controls(caller_perms(), this))
      return E_PERM;
    elseif (this.location != $mail_agent)
      "... we don't care...";
      return pass(@args);
    elseif (index(name, " "))
      "... we don't care...";
    elseif (rp = $mail_agent:reserved_pattern(name))
      player:tell("Mailing list name \"", a, "\" uses a reserved pattern: ", rp[1]);
      return 0;
    elseif (valid(p = $mail_agent:match(name, #-1)) && (p != this && name in p.aliases))
      player:tell("Mailing list name \"", name, "\" in use on ", p.name, "(", p, ")");
      return 0;
    endif
    return pass(name);
  endverb

  verb ok_annotate (this none this) owner: #2 flags: "rxd"
    ":ok_annotate(caller,callerperms) => true iff caller can do annotations";
    return args[1] in {this, $mail_agent} || (args[2].wizard || this:is_annotatable_by(args[2]));
  endverb

  verb annotate_message_seq (this none this) owner: #2 flags: "rxd"
    "annotate_message_seq(note, \"append\"|\"prepend\", message_seq) ;";
    "";
    "Prepend or append (default is prepend) note (a list of strings) to each message in message_seq";
    "Recipient must be annotatable (:is_annotatable_by() returns 1) by the caller for this to work.";
    {note, appendprepend, message_seq} = args;
    if (!this:ok_annotate(caller, caller_perms()))
      return E_PERM;
    endif
    for i in ($seq_utils:tolist(message_seq))
      body = this:message_body_by_index(i);
      if (appendprepend == "append")
        body = {@body, "", @note};
      else
        body = {@note, "", @body};
      endif
      this:set_message_body_by_index(i, body);
    endfor
    return 1;
    "Copied from annotatetest (#87053):annotate_message_seq [verb author Puff (#1449)] at Mon Feb 14 14:04:56 2005 PST";
  endverb
endobject