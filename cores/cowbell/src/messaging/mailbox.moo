object MAILBOX [
  import_export_id -> "mailbox",
  import_export_hierarchy -> {"messaging"}
]
  name: "Generic Mailbox"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  override aliases (owner: HACKER, flags: "rc") = {"mailbox"};
  override description (owner: HACKER, flags: "rc") = "A sturdy mailbox for receiving letters.";
  override object_documentation (owner: HACKER, flags: "rc") = "A mailbox holds letters for its owner. Anyone can deposit letters, but only the owner can view or take them.";

  method acceptable owner: ARCH_WIZARD
    "Check if an object can be deposited. Only letters are accepted.";
    {what} = args;
    return isa(what, $letter);
  endmethod

  verb deposit (any in this) owner: ARCH_WIZARD flags: "rxd"
    "Deposit a held letter and notify the mailbox owner.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (!valid(dobj) || !isa(dobj, $letter))
      player:inform_current($event:mk_error(player, "You can only deposit letters in a mailbox."));
      return;
    endif
    if (dobj.location != player)
      player:inform_current($event:mk_error(player, "You're not holding that letter."));
      return;
    endif
    if (!this:acceptable(dobj))
      player:inform_current($event:mk_error(player, "This mailbox does not accept that letter."));
      return;
    endif
    "The acceptance callback may suspend or transfer the letter.";
    if (dobj.location != player)
      player:inform_current($event:mk_error(player, "You're not holding that letter."));
      return;
    endif
    move(dobj, this);
    player:inform_current($event:mk_info(player, "You deposit ", dobj.name, " into the mailbox."));
    const recipient = this.owner;
    if (valid(recipient) && recipient != player && recipient in connected_players())
      `recipient:tell($event:mk_info(recipient, "You have new mail from ", player.name, "."):with_audience('utility)) ! ANY';
    endif
  endverb

  method unread_count owner: ARCH_WIZARD
    "Count letters that haven't been read yet.";
    count = 0;
    for letter in (this.contents)
      if (isa(letter, $letter) && letter.read_at == 0)
        count = count + 1;
      endif
    endfor
    return count;
  endmethod

  method look_self owner: ARCH_WIZARD
    "Describe the mailbox with letter listing for owner.";
    set_task_perms(caller_perms());
    desc = this.description;
    letters = {};
    for item in (this.contents)
      if (isa(item, $letter))
        letters = {@letters, item};
      endif
    endfor
    count = length(letters);
    "Non-owners just see basic info";
    if (caller_perms() != this.owner && !caller_perms().wizard)
      desc = desc + " It belongs to " + this.owner.name + ".";
      return <$look, .what = this, .title = this:name(), .description = desc>;
    endif
    "Owner sees letter table";
    if (count == 0)
      desc = desc + " It is empty.";
      return <$look, .what = this, .title = this:name(), .description = desc>;
    endif
    "Build table of letters";
    headers = {"#", "Status", "From", "Subject"};
    rows = {};
    idx = 1;
    for letter in (letters)
      status = letter.read_at == 0 ? "New" | "Read";
      from_name = valid(letter.author) ? letter.author.name | "anonymous";
      subject = letter.name != "letter" ? letter.name | "(no subject)";
      rows = {@rows, {tostr(idx), status, from_name, subject}};
      idx = idx + 1;
    endfor
    unread = this:unread_count();
    summary = tostr(count, " letter", count == 1 ? "" | "s");
    if (unread > 0)
      summary = summary + tostr(" (", unread, " unread)");
    endif
    "Build content";
    parts = {desc, "", $format.title:mk(summary), $format.table:mk(headers, rows), "", "Use: read <#> from mailbox"};
    content = $format.block:mk(@parts);
    return <$look, .what = this, .title = this:name(), .description = content>;
  endmethod

  verb read (any from this) owner: ARCH_WIZARD flags: "rxd"
    "Read a letter from the mailbox by number. Usage: read <#> from mailbox";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (player != this.owner && !player.wizard)
      player:inform_current($event:mk_error(player, "This isn't your mailbox."));
      return;
    endif
    "Get letters list";
    let letters = {};
    for item in (this.contents)
      if (isa(item, $letter))
        letters = {@letters, item};
      endif
    endfor
    if (length(letters) == 0)
      player:inform_current($event:mk_info(player, "The mailbox is empty."));
      return;
    endif
    "Parse the number - strip # if present";
    let num_str = dobjstr;
    if (num_str && num_str[1] == "#")
      num_str = num_str[2..$];
    endif
    let idx = toint(num_str);
    if (idx < 1 || idx > length(letters))
      player:inform_current($event:mk_error(player, "Invalid letter number. Use 1-", tostr(length(letters)), "."));
      return;
    endif
    "Read the letter via action_read";
    const letter = letters[idx];
    letter:action_read(player, []);
  endverb

  verb "take get" (any from this) owner: ARCH_WIZARD flags: "rxd"
    "Take a letter from this mailbox. Owner only.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (player != this.owner && !player.wizard)
      player:inform_current($event:mk_error(player, "This isn't your mailbox."));
      return;
    endif
    "Match the object name within the mailbox contents";
    const target = $match:resolve_in_scope(dobjstr, this.contents);
    if (!valid(target))
      if (target == $failed_match)
        player:inform_current($event:mk_error(player, "I don't see that in the mailbox."));
      elseif (target == $ambiguous_match)
        player:inform_current($event:mk_error(player, "Which one do you mean?"));
      else
        player:inform_current($event:mk_error(player, "Take what from the mailbox?"));
      endif
      return;
    endif
    "Literal IDs must not bypass the mailbox boundary, even if matching changes.";
    if (target.location != this || !isa(target, $letter))
      player:inform_current($event:mk_error(player, "I don't see that letter in the mailbox."));
      return;
    endif
    move(target, player);
    player:inform_current($event:mk_info(player, "You take ", target.name, " from the mailbox."));
  endverb

  verb mail (this none none) owner: ARCH_WIZARD flags: "rxd"
    "List the letters in this mailbox. Owner only.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (player != this.owner && !player.wizard)
      player:inform_current($event:mk_error(player, "This isn't your mailbox."));
      return;
    endif
    let letters = {};
    for item in (this.contents)
      if (isa(item, $letter))
        letters = {@letters, item};
      endif
    endfor
    if (length(letters) == 0)
      player:inform_current($event:mk_info(player, "Your mailbox is empty."));
      return;
    endif
    "Build table of letters";
    const headers = {"#", "Status", "From", "Subject"};
    let rows = {};
    let idx = 1;
    for letter in (letters)
      const status = letter.read_at == 0 ? "NEW" | "";
      const from_name = valid(letter.author) ? letter.author.name | "anonymous";
      const subject = letter.name != "letter" ? letter.name | "(no subject)";
      rows = {@rows, {tostr(idx), status, from_name, subject}};
      idx = idx + 1;
    endfor
    const unread = this:unread_count();
    let summary = tostr(length(letters), " letter", length(letters) == 1 ? "" | "s");
    if (unread > 0)
      summary = summary + tostr(" (", unread, " unread)");
    endif
    "Build and display";
    const parts = {$format.title:mk("Mailbox: " + summary), $format.table:mk(headers, rows), "", "Use: read <#> from mailbox"};
    const content = $format.block:mk(@parts);
    let event = $event:mk_info(player, content):with_presentation_hint('inset);
    player:inform_current(event);
  endverb
endobject
