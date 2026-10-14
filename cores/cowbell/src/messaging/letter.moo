object LETTER [
  import_export_id -> "letter",
  import_export_hierarchy -> {"messaging"}
]
  name: "letter"
  parent: NOTE
  location: PROTOTYPE_BOX
  owner: TEST_PLAYER
  fertile: true
  readable: true

  property subject (owner: TEST_PLAYER, flags: "rc") = "";
  property addressee (owner: TEST_PLAYER, flags: "rc") = #-1;
  property author (owner: TEST_PLAYER, flags: "rc") = #-1;
  property read_at (owner: TEST_PLAYER, flags: "rc") = 0;
  property sealed (owner: TEST_PLAYER, flags: "rc") = 0;
  property sent_at (owner: TEST_PLAYER, flags: "rc") = 0;

  override aliases (owner: TEST_PLAYER, flags: "rc") = {"letter"};
  override object_documentation (owner: TEST_PLAYER, flags: "rc") = "A letter is a note with a subject, author, addressee, timestamps, and sealing. The subject is message content; the object name remains letter. A sealed letter can be read by its author or addressee.";

  method can_read owner: ARCH_WIZARD
    "Report whether the accessor can read an open or sealed letter.";
    const {accessor} = args;
    if (!this.sealed)
      return pass(@args);
    endif
    if (valid(accessor) && (accessor == this.addressee || accessor == this.author))
      return ['allowed -> true, 'reason -> {}];
    endif
    return ['allowed -> false, 'reason -> "This letter is sealed and addressed to someone else."];
  endmethod

  method can_write owner: ARCH_WIZARD
    "Preserve open-note rules; restrict sealed edits to the author or owner.";
    const {accessor} = args;
    if (!this.sealed)
      return pass(@args);
    endif
    if (valid(accessor) && (accessor == this.author || accessor == this.owner))
      return ['allowed -> true, 'reason -> {}];
    endif
    return ['allowed -> false, 'reason -> "This letter is sealed."];
  endmethod

  method action_read owner: ARCH_WIZARD
    "Read privately for the authenticated accessor.";
    const {who, context} = args;
    const actor = caller_perms();
    actor == who || actor.wizard || raise(E_PERM);
    !this:can_read(who)['allowed] && return false;
    return this:do_read(who, true);
  endmethod

  method do_read owner: ARCH_WIZARD
    "Display the letter with metadata and record when it was read.";
    caller != this && raise(E_PERM, "do_read must be called by this object");
    {who, ?silent = false} = args;
    "Record first read time";
    if (this.read_at == 0)
      this.read_at = time();
    endif
    "Build letter display";
    parts = {};
    "Title/subject";
    subject = this.subject ? this.subject | "(no subject)";
    parts = {@parts, $format.title:mk(subject)};
    "From/To metadata";
    meta_lines = {};
    if (valid(this.author))
      meta_lines = {@meta_lines, "From: " + this.author.name};
    endif
    if (valid(this.addressee))
      meta_lines = {@meta_lines, "To: " + this.addressee.name};
    endif
    if (this.sent_at > 0)
      meta_lines = {@meta_lines, "Sent: " + ctime(this.sent_at)};
    endif
    if (length(meta_lines) > 0)
      parts = {@parts, meta_lines:join(" | ")};
      parts = {@parts, ""};
    endif
    "Content";
    text = this.text;
    if (length(text) == 0)
      parts = {@parts, "(blank)"};
    else
      parts = {@parts, $format:authored(this.content_kind, text:join("\n"))};
    endif
    "Display";
    content = $format.block:mk(@parts);
    event = $event:mk_info(who, content):with_presentation_hint('inset);
    who:inform_current(event);
    "Announce to room";
    if (!silent && valid(who.location))
      room_event = $event:mk_info(who, @this.read_msg):with_dobj(this):with_this(who.location);
      `who.location:announce(room_event) ! E_VERBNF';
    endif
    this:fire_trigger('on_read, ['Actor -> who]);
    return true;
  endmethod

  verb seal (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Seal this letter so only the addressee can read it.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    "Only author or owner can seal";
    if (this.author != player && this.owner != player && !player.wizard)
      event = $event:mk_error(player, "You didn't write this letter.");
      player:inform_current(event);
      return;
    endif
    if (this.sealed)
      event = $event:mk_info(player, "The letter is already sealed.");
      player:inform_current(event);
      return;
    endif
    this.sealed = true;
    event = $event:mk_info(player, "You seal the letter.");
    player:inform_current(event);
  endverb

  verb "unseal open" (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Unseal this letter, making it readable by anyone.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    "Only author, addressee, or owner can unseal";
    if (this.author != player && this.addressee != player && this.owner != player && !player.wizard)
      event = $event:mk_error(player, "This letter isn't yours to open.");
      player:inform_current(event);
      return;
    endif
    if (!this.sealed)
      event = $event:mk_info(player, "The letter is already open.");
      player:inform_current(event);
      return;
    endif
    this.sealed = false;
    event = $event:mk_info(player, "You break the seal on the letter.");
    player:inform_current(event);
  endverb

  method action_write owner: ARCH_WIZARD
    "Write privately for the authenticated accessor.";
    const {who, context, line} = args;
    const actor = caller_perms();
    actor == who || actor.wizard || raise(E_PERM);
    !this:can_write(who)['allowed] && return false;
    return this:do_write(who, line, true);
  endmethod

  method do_write owner: ARCH_WIZARD
    "Append through the note helper after checking the object-local caller.";
    caller == this || raise(E_PERM);
    const {who, text, ?silent = false} = args;
    !this:can_write(who)['allowed] && return false;
    if (!valid(this.author))
      this.author = who;
    endif
    return pass(who, text, silent);
  endmethod

  verb address (this any any) owner: ARCH_WIZARD flags: "rxd"
    "Address this letter to someone, sealing it for their eyes only.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    "Usage: address <letter> to <player>";
    "Use match_player to find players anywhere, not just nearby";
    const recipient = iobjstr ? $match:match_player(iobjstr) | #-1;
    if (!valid(recipient) || !is_player(recipient))
      if (iobjstr)
        event = $event:mk_error(player, "No player named '", iobjstr, "' found.");
      else
        event = $event:mk_error(player, "Address the letter to whom?");
      endif
      player:inform_current(event);
      return;
    endif
    "Only author or owner can address";
    if (this.author != player && this.owner != player && !player.wizard)
      event = $event:mk_error(player, "You didn't write this letter.");
      player:inform_current(event);
      return;
    endif
    this.addressee = recipient;
    this.sealed = true;
    this.sent_at = time();
    "Set author if not yet set";
    if (!valid(this.author))
      this.author = player;
    endif
    event = $event:mk_info(player, "You address the letter to ", $sub:reference(recipient), " and seal it.");
    player:inform_current(event);
  endverb

  method look_self owner: ARCH_WIZARD
    "Describe the public envelope and authorized writing hint.";
    const actor = caller_perms();
    let parts = {this.sealed ? "It is sealed." | "It is open."};
    if (valid(this.author))
      parts = {@parts, "From: " + this.author.name};
    endif
    if (valid(this.addressee))
      parts = {@parts, "To: " + this.addressee.name};
    endif
    let description = this:description();
    if (this:can_read(actor)['allowed] && length(this.text) > 0)
      description = $format.block:mk(description, "There appears to be some writing on it.");
    endif
    description = $format.block:mk(description, parts:join(" "));
    return <$look, .what = this, .title = this:name(), .description = description>;
  endmethod

  verb reply (none any this) owner: ARCH_WIZARD flags: "rxd"
    "Create a new letter addressed to this letter's author.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    "Usage: reply to <letter>";
    if (!valid(this.author))
      const event = $event:mk_error(player, "This letter has no author to reply to.");
      player:inform_current(event);
      return;
    endif
    "Create new letter";
    const new_letter = create($letter, player);
    new_letter.name = "letter";
    new_letter.subject = this.subject ? "Re: " + this.subject | "";
    new_letter.addressee = this.author;
    new_letter.author = player;
    "Move to player";
    move(new_letter, player);
    const event = $event:mk_info(player, "You prepare a reply to ", $sub:reference(this.author), ". Write on it to compose your response.");
    player:inform_current(event);
  endverb

  verb edit (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Open text editor to edit this letter.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    const check = this:can_write(player);
    if (!check['allowed])
      player:inform_current($event:mk_error(player, check['reason]));
      return;
    endif
    const conn = connection();
    const session_id = player:start_edit_session(this, "receive_edit", {conn});
    const editor_title = "Edit: " + (this.subject ? this.subject | "(no subject)");
    const current_body = this.text:join("\n");
    present(conn, session_id, "text/djot", "text-editor", current_body, {{"object", $url_utils:to_curie_str(this)}, {"verb", "receive_edit"}, {"title", editor_title}, {"text_mode", "string"}, {"session_id", session_id}});
  endverb

  method receive_edit owner: ARCH_WIZARD
    "Save the bound editor session while its authenticated writer remains authorized.";
    const {session_id, content} = args;
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    const session = player:get_edit_session(session_id);
    session['target] == this && session['verb] == "receive_edit" || raise(E_PERM);
    const {conn} = session['args];
    "HTTP saves have no connection view; authorize them through the player and stored session.";
    const request_conn = `connection() ! E_INVARG => #-1';
    request_conn == #-1 || request_conn == conn || raise(E_PERM);
    request_conn == #-1 || conn in { entry[1] for entry in (connections(player)) } || raise(E_PERM);
    if (content == 'close)
      player:end_edit_session(session_id);
      return;
    endif
    typeof(content) == TYPE_STR || raise(E_TYPE);
    this:can_write(player)['allowed] || raise(E_PERM);
    "Policy evaluation may suspend; recheck the session and connection before mutation.";
    player:get_edit_session(session_id) == session || raise(E_PERM);
    request_conn == #-1 || conn in { current_entry[1] for current_entry in (connections(player)) } || raise(E_PERM);
    $format:authored(this.content_kind, content);
    this.text = content:split("\n");
    if (!valid(this.author))
      this.author = player;
    endif
    "HTTP editors display their save result; connected callers also receive a local message.";
    request_conn != #-1 && player:inform_connection(conn, $event:mk_info(player, "Letter saved."));
  endmethod
endobject
