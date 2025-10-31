object LETTER
  name: "generic letter"
  parent: NOTE
  owner: #2
  fertile: true
  readable: true

  property burn_failed_msg (owner: #2, flags: "rc") = "%T might be damp.  In any case, %[tps] won't burn.";
  property burn_succeeded_msg (owner: #2, flags: "rc") = "%T burns with a smokeless flame and leaves no ash.";
  property oburn_failed_msg (owner: #2, flags: "rc") = 0;
  property oburn_succeeded_msg (owner: #2, flags: "rc") = "stares at %t; %[tps] bursts into flame and disappears, leaving no ash.";

  override aliases (owner: HACKER, flags: "rc") = {"generic letter"};
  override description (owner: HACKER, flags: "rc") = "Some writing on the letter explains that you should 'read letter', and when you've finished, 'burn letter'.";
  override encryption_key (owner: HACKER, flags: "c");
  override import_export_id = "letter";
  override key (owner: HACKER, flags: "c");
  override object_size = {2373, 1084848672};
  override otake_failed_msg (owner: HACKER, flags: "rc");
  override otake_succeeded_msg (owner: HACKER, flags: "rc");
  override take_failed_msg (owner: HACKER, flags: "rc") = "This is a private letter.";
  override take_succeeded_msg (owner: HACKER, flags: "rc");

  verb burn (this none none) owner: #2 flags: "rd"
    who = valid(caller_perms()) ? caller_perms() | player;
    if ($perm_utils:controls(who, this) || this:is_readable_by(who))
      result = this:do_burn();
    else
      result = 0;
    endif
    player:tell(result ? this:burn_succeeded_msg() | this:burn_failed_msg());
    if (msg = result ? this:oburn_succeeded_msg() | this:oburn_failed_msg())
      player.location:announce(player.name, " ", msg);
    endif
  endverb

  verb "burn_succeeded_msg oburn_succeeded_msg burn_failed_msg oburn_failed_msg" (this none this) owner: #2 flags: "rxd"
    return (msg = this.(verb)) ? $string_utils:pronoun_sub(msg) | "";
  endverb

  verb do_burn (this none this) owner: #2 flags: "rxd"
    if (this != $letter && (caller == this || $perm_utils:controls(caller_perms(), this)))
      fork (0)
        $recycler:_recycle(this);
      endfork
      return 1;
    else
      return E_PERM;
    endif
  endverb
endobject