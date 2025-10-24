object NEW_PLAYER_LOG
  name: "Player-Creation-Log"
  parent: BIG_MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: #2

  override aliases (owner: HACKER, flags: "r") = {"Player-Creation-Log", "PCL"};
  override description = "Log of player creations.";
  override mail_forward = {};
  override mail_notify = {#2};
  override moderated = {NEW_PLAYER_LOG};
  override object_size = {3172, 1084848672};
  override summary_uses_body = 1;

  verb display_seq_headers (this none this) owner: #2 flags: "rxd"
    ":display_seq_headers(msg_seq[,cur])";
    if (!this:ok(caller, caller_perms()))
      return E_PERM;
    endif
    player:tell("       WHEN    BY        WHO                 EMAIL-ADDRESS");
    pass(@args);
  endverb

  verb msg_summary_line (this none this) owner: #2 flags: "rxd"
    when = ctime(args[1])[5..10];
    from = args[2];
    by = $string_utils:left(from[1..index(from, " (") - 1], -9);
    subject = args[4];
    who = subject[1..(open = index(subject, " (")) - 1];
    if ((close = rindex(subject, ")")) > open)
      who = who[1..min(9, $)] + subject[open..close];
    endif
    who = $string_utils:left(who, 18);
    line = args[("" in args) + 1];
    email = line[1..index(line + " ", " ") - 1];
    if (!index(email, "@"))
      email = "??";
    endif
    return tostr(when, "  ", by, " ", who, "  ", email);
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.mail_notify = {player};
      player:set_current_message(this, 0, 0, 1);
      this.moderated = {this};
    else
      return E_PERM;
    endif
  endverb

  verb is_usable_by (this none this) owner: #2 flags: "rxd"
    "Copied from Generic Mail Recipient (#6419):is_usable_by by Rog (#4292) Tue Mar  2 10:02:32 1993 PST";
    return !this.moderated || (this:is_writable_by(who = args[1]) || who in this.moderated || who.wizard);
  endverb

  verb expire_old_messages (none none none) owner: #2 flags: "rxd"
    "Stop breaking the expire task completely with out of seconds/ticks.";
    if (this:ok_write(caller, caller_perms()))
      fork (0)
        pass(@args);
      endfork
    else
      return E_PERM;
    endif
  endverb
endobject