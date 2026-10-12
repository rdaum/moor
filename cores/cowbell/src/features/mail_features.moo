object MAIL_FEATURES [
  import_export_id -> "mail_features",
  import_export_hierarchy -> {"features"}
]
  name: "Mail Features"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Provides mail commands (mail, send, compose) for players.";

  verb mail (none none none) owner: ARCH_WIZARD flags: "rxd"
    "HINT: -- Check your mailbox for letters.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let mailbox = player:find_mailbox();
    if (!valid(mailbox))
      "Create mailbox in mail room";
      mailbox = create($mailbox, player);
      mailbox.name = player.name + "'s mailbox";
      move(mailbox, $mail_room);
      player:inform_current($event:mk_info(player, "A new mailbox has been set up for you in the Mail Room."));
    endif
    mailbox:mail();
  endverb

  verb compose (any any any) owner: ARCH_WIZARD flags: "rxd"
    "HINT: [<subject>] [to <player>] -- Create a new letter for writing.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    const letter = create($letter, player);
    "Set subject if provided";
    if (dobjstr && dobjstr != "")
      letter.name = dobjstr;
    else
      letter.name = "letter";
    endif
    "Set addressee if 'to' preposition used";
    if (prepstr == "to" && iobjstr)
      "Use match_player to find players anywhere, not just nearby";
      const recipient = $match:match_player(iobjstr);
      if (valid(recipient) && is_player(recipient))
        letter.addressee = recipient;
        letter.author = player;
      elseif (iobjstr)
        player:inform_current($event:mk_error(player, "No player named '", iobjstr, "' found."));
      endif
    endif
    move(letter, player);
    "Build confirmation message";
    if (valid(letter.addressee))
      if (letter.name != "letter")
        player:inform_current($event:mk_info(player, "You have a letter titled '", letter.name, "' addressed to ", letter.addressee.name, ". Write on it to compose your message."));
      else
        player:inform_current($event:mk_info(player, "You have a letter addressed to ", letter.addressee.name, ". Write on it to compose your message."));
      endif
    else
      player:inform_current($event:mk_info(player, "You have a fresh letter ready to write on."));
    endif
  endverb

  verb send (any any any) owner: ARCH_WIZARD flags: "rxd"
    "HINT: <letter> [to <player>] -- Send a letter to someone.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (!valid(dobj) || !isa(dobj, $letter))
      player:inform_current($event:mk_error(player, "Send what letter?"));
      return;
    endif
    if (dobj.location != player)
      player:inform_current($event:mk_error(player, "You're not holding that letter."));
      return;
    endif
    "Determine recipient - explicit or from letter";
    let recipient = #-1;
    if (prepstr == "to" && iobjstr)
      "Use match_player to find players anywhere, not just nearby";
      recipient = $match:match_player(iobjstr);
      if (!valid(recipient) || !is_player(recipient))
        player:inform_current($event:mk_error(player, "No player named '", iobjstr, "' found."));
        return;
      endif
    elseif (valid(dobj.addressee) && is_player(dobj.addressee))
      recipient = dobj.addressee;
    else
      player:inform_current($event:mk_error(player, "Send the letter to whom? Use: ", $format.code:inline("send <letter> to <player>")));
      return;
    endif
    "Find recipient's mailbox";
    let recipient_mailbox = recipient:find_mailbox();
    if (!valid(recipient_mailbox))
      player:inform_current($event:mk_error(player, recipient.name, " doesn't have a mailbox."));
      return;
    endif
    "Mailbox lookup may suspend or change custody; validate its result without callbacks.";
    if (dobj.location != player)
      player:inform_current($event:mk_error(player, "You're not holding that letter."));
      return;
    endif
    isa(recipient_mailbox, $mailbox) && recipient_mailbox.owner == recipient || raise(E_PERM);
    move(dobj, recipient_mailbox);
    dobj.location == recipient_mailbox && recipient_mailbox.owner == recipient || raise(E_PERM);
    "Address and seal only the successfully delivered letter.";
    dobj.addressee = recipient;
    dobj.sealed = true;
    dobj.sent_at = time();
    if (!valid(dobj.author))
      dobj.author = player;
    endif
    player:inform_current($event:mk_info(player, "You send ", dobj.name, " to ", recipient.name, "."));
    "Notify recipient if online";
    if (recipient in connected_players() && recipient != player)
      const event = $event:mk_info(recipient, "You have new mail from ", $format.annotation:object(player), "."):as_inset();
      `recipient:tell(event) ! ANY';
    endif
  endverb

  method help_topics owner: ARCH_WIZARD
    "Return help topics for mail commands.";
    {for_player, ?topic = ""} = args;
    "Main overview topic";
    overview = $help:mk("mail", "Mail commands", "Send and receive letters from other players:\n\n`mail` - Check your mailbox\n`compose` - Create a new letter\n`send` - Send a letter to someone\n\nLetters can be written on, sealed, and delivered to mailboxes.", {"letters", "mailbox"}, 'mail, {});
    "If asking for all topics, just return overview";
    topic == "" && return {overview};
    "Check if topic matches overview";
    overview:matches(topic) && return overview;
    "Try to generate help from verb HINT tags";
    verb_help = `$help_utils:verb_help_from_hint(this, topic, 'mail) ! ANY => 0';
    typeof(verb_help) != TYPE_INT && return verb_help;
    return 0;
  endmethod
endobject
