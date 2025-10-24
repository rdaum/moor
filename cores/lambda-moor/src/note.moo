object NOTE
  name: "generic note"
  parent: THING
  owner: #2
  fertile: true
  readable: true

  property encryption_key (owner: #2, flags: "c") = 0;
  property text (owner: #2, flags: "c") = {};
  property writers (owner: #2, flags: "rc") = {};

  override aliases = {"generic note"};
  override description = "There appears to be some writing on the note ...";
  override object_size = {6265, 1084848672};

  verb "r*ead" (this none none) owner: #2 flags: "rxd"
    if (!this:is_readable_by(valid(caller_perms()) ? caller_perms() | player))
      player:tell("Sorry, but it seems to be written in some code that you can't read.");
    else
      this:look_self();
      player:tell();
      player:tell_lines_suspended(this:text());
      player:tell();
      player:tell("(You finish reading.)");
    endif
  endverb

  verb "er*ase" (this none none) owner: #2 flags: "rxd"
    if (this:is_writable_by(valid(caller_perms()) ? caller_perms() | player))
      this:set_text({});
      player:tell("Note erased.");
    else
      player:tell("You can't erase this note.");
    endif
  endverb

  verb "wr*ite" (any on this) owner: #2 flags: "rxd"
    if (this:is_writable_by(valid(caller_perms()) ? caller_perms() | player))
      this:set_text({@this.text, dobjstr});
      player:tell("Line added to note.");
    else
      player:tell("You can't write on this note.");
    endif
  endverb

  verb "del*ete rem*ove" (any from this) owner: #2 flags: "rd"
    if (!this:is_writable_by(player))
      player:tell("You can't modify this note.");
    elseif (!dobjstr)
      player:tell("You must tell me which line to delete.");
    else
      line = toint(dobjstr);
      if (line < 0)
        line = line + length(this.text) + 1;
      endif
      if (line <= 0 || line > length(this.text))
        player:tell("Line out of range.");
      else
        this:set_text(listdelete(this.text, line));
        player:tell("Line deleted.");
      endif
    endif
  endverb

  verb encrypt (this with any) owner: #2 flags: "rd"
    set_task_perms(player);
    key = $lock_utils:parse_keyexp(iobjstr, player);
    if (typeof(key) == STR)
      player:tell("That key expression is malformed:");
      player:tell("  ", key);
    else
      try
        this.encryption_key = key;
        player:tell("Encrypted ", this.name, " with this key:");
        player:tell("  ", $lock_utils:unparse_key(key));
      except error (ANY)
        player:tell(error[2], ".");
      endtry
    endif
  endverb

  verb decrypt (this none none) owner: #2 flags: "rd"
    set_task_perms(player);
    try
      dobj.encryption_key = 0;
      player:tell("Decrypted ", dobj.name, ".");
    except error (ANY)
      player:tell(error[2], ".");
    endtry
  endverb

  verb text (this none this) owner: #2 flags: "rxd"
    cp = caller_perms();
    if ($perm_utils:controls(cp, this) || this:is_readable_by(cp))
      return this.text;
    else
      return E_PERM;
    endif
  endverb

  verb is_readable_by (this none this) owner: #2 flags: "rxd"
    key = this.encryption_key;
    return key == 0 || $lock_utils:eval_key(key, args[1]);
  endverb

  verb set_text (this none this) owner: #2 flags: "rxd"
    cp = caller_perms();
    newtext = args[1];
    if ($perm_utils:controls(cp, this) || this:is_writable_by(cp))
      if (typeof(newtext) == LIST)
        this.text = newtext;
      else
        return E_TYPE;
      endif
    else
      return E_PERM;
    endif
  endverb

  verb is_writable_by (this none this) owner: #2 flags: "rxd"
    who = args[1];
    wr = this.writers;
    if ($perm_utils:controls(who, this))
      return 1;
    elseif (typeof(wr) == LIST)
      return who in wr;
    else
      return wr;
    endif
  endverb

  verb "mailme @mailme" (this none none) owner: #2 flags: "rd"
    "Usage:  mailme <note>";
    "  uses $network to sends the text of this note to your REAL internet email address.";
    if (!this:is_readable_by(player))
      return player:tell("Sorry, but it seems to be written in some code that you can't read.");
    elseif (!(email = $wiz_utils:get_email_address(player)))
      return player:tell("Sorry, you don't have a registered email address.");
    elseif (!$network.active)
      return player:tell("Sorry, internet mail is disabled.");
    elseif (!(text = this:text()))
      return player:tell($string_utils:pronoun_sub("%T is empty--there wouldn't be any point to mailing it."));
    endif
    player:tell("Mailing ", this:title(), " to ", email, ".");
    player:tell("... ", length(text), " lines ...");
    suspend(0);
    $network:sendmail(email, this:titlec(), "", @text);
  endverb
endobject