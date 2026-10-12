object NOTE [
  import_export_id -> "note",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Note"
  parent: THING
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property content_kind (owner: ARCH_WIZARD, flags: "rc") = 'literal;
  property erase_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "erase", .for_others = "erases">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property read_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {"You can't read that."};
  property read_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "read", .for_others = "reads">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property read_rule (owner: ARCH_WIZARD, flags: "rc") = 0;
  property text (owner: ARCH_WIZARD, flags: "c") = {};
  property write_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {"You can't write on that."};
  property write_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "write", .for_others = "writes">,
    " on ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property write_rule (owner: ARCH_WIZARD, flags: "rc") = 0;

  override description (owner: ARCH_WIZARD, flags: "rc") = "A blank note, ready to be written on.";
  override object_documentation (owner: ARCH_WIZARD, flags: "rc") = {
    "# Notes",
    "",
    "## Overview",
    "",
    "Notes are readable objects that can hold text content. They serve as the base for letters, signs, books, scrolls, and any other text-bearing items.",
    "",
    "## Properties",
    "",
    "### text",
    "",
    "List of strings containing the note's content. Each element is one line.",
    "",
    "### text_type",
    "",
    "Content type: 'plain' (default) or 'djot' for rich text formatting.",
    "",
    "### Access Control Rules",
    "",
    "- `read_rule`: Controls who can read the note (default: `0` = public)",
    "- `write_rule`: Controls who can write/erase (default: `0` = public)",
    "- `read_denied_msg`: Message shown when read is denied",
    "- `write_denied_msg`: Message shown when write is denied",
    "",
    "Set rules using the `@set-rule` command:",
    "",
    "```",
    "@set-rule letter.read_rule This owner_is(Accessor)?",
    "@set-rule journal.write_rule This owner_is(Accessor)?",
    "```",
    "",
    "## Commands",
    "",
    "### read <note>",
    "",
    "Display the note's contents.",
    "",
    "### write <text> on <note>",
    "",
    "Append a line of text to the note.",
    "",
    "### erase <note>",
    "",
    "Clear all text from the note.",
    "",
    "### delete <line#> from <note>",
    "",
    "Remove a specific line. Negative numbers count from end (-1 = last line).",
    "",
    "## Example: Creating a Private Letter",
    "",
    "```moo",
    "letter = create($note);",
    "letter.name = \"a sealed letter\";",
    "letter.description = \"A letter sealed with red wax.\";",
    "letter:set_text({\"Dear friend,\", \"\", \"Meet me at midnight.\", \"\", \"- A\"});",
    "@set-rule letter.read_rule This owner_is(Accessor)?",
    "```",
    "",
    "## Example: Creating a Public Sign",
    "",
    "```moo",
    "sign = create($note);",
    "sign.name = \"a wooden sign\";",
    "sign.description = \"A weathered wooden sign.\";",
    "sign:set_text({\"Welcome to Cowbell!\", \"Population: Growing\"});",
    "@set-rule sign.get_rule NOT true",
    "```"
  };

  method can_read owner: ARCH_WIZARD
    "Check if accessor can read this note. Returns {allowed, reason}.";
    {accessor} = args;
    "No rule = public access";
    if (this.read_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate read rule";
    result = $rule_engine:evaluate(this.read_rule, ['This -> this, 'Accessor -> accessor]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      return ['allowed -> false, 'reason -> this.read_denied_msg];
    endif
  endmethod

  method can_write owner: ARCH_WIZARD
    "Check if accessor can write on this note. Returns {allowed, reason}.";
    {accessor} = args;
    "No rule = public access";
    if (this.write_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate write rule";
    result = $rule_engine:evaluate(this.write_rule, ['This -> this, 'Accessor -> accessor]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      return ['allowed -> false, 'reason -> this.write_denied_msg];
    endif
  endmethod

  method do_read owner: ARCH_WIZARD
    "Core: display the note's text to the reader.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_read must be called by this object");
    {who, ?silent = false} = args;
    text = this.text;
    if (length(text) == 0)
      event = $event:mk_info(who, "The note is blank.");
      who:inform_current(event);
      return true;
    endif
    "Display the text";
    content = $format:authored(this.content_kind, text:join("\n"));
    event = $event:mk_info(who, content);
    who:inform_current(event);
    if (!silent && valid(who.location))
      event = $event:mk_info(who, @this.read_msg):with_dobj(this):with_this(who.location);
      `who.location:announce(event) ! E_VERBNF';
    endif
    this:fire_trigger('on_read, ['Actor -> who]);
    return true;
  endmethod

  method do_write owner: ARCH_WIZARD
    "Core: append a line to the note.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_write must be called by this object");
    {who, line, ?silent = false} = args;
    const updated = {@this.text, line};
    $format:authored(this.content_kind, updated:join("\n"));
    this.text = updated;
    if (!silent && valid(who.location))
      event = $event:mk_info(who, @this.write_msg):with_dobj(this):with_this(who.location);
      `who.location:announce(event) ! E_VERBNF';
    endif
    this:fire_trigger('on_write, ['Actor -> who, 'Line -> line]);
    return true;
  endmethod

  method do_erase owner: ARCH_WIZARD
    "Core: erase the note's text.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_erase must be called by this object");
    {who, ?silent = false} = args;
    this.text = {};
    if (!silent && valid(who.location))
      event = $event:mk_info(who, @this.erase_msg):with_dobj(this):with_this(who.location);
      `who.location:announce(event) ! E_VERBNF';
    endif
    this:fire_trigger('on_erase, ['Actor -> who]);
    return true;
  endmethod

  verb "read r*ead" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Read this note";
    set_task_perms(caller_perms());
    "Check access via rule";
    access_check = this:can_read(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_read(player);
  endverb

  verb "erase er*ase" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Erase this note";
    set_task_perms(caller_perms());
    "Check access via rule";
    access_check = this:can_write(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_erase(player);
  endverb

  verb "delete del*ete remove rem*ove" (any from this) owner: ARCH_WIZARD flags: "rd"
    "Delete a line from this note";
    set_task_perms(caller_perms());
    "Check access via rule";
    access_check = this:can_write(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    if (!dobjstr)
      event = $event:mk_error(player, "You must specify which line to delete.");
      player:inform_current(event);
      return;
    endif
    line = toint(dobjstr);
    text = this.text;
    text_len = length(text);
    "Support negative indexing";
    if (line < 0)
      line = text_len + line + 1;
    endif
    if (line <= 0 || line > text_len)
      event = $event:mk_error(player, "Line out of range.");
      player:inform_current(event);
      return;
    endif
    const updated = listdelete(text, line);
    $format:authored(this.content_kind, updated:join("\n"));
    this.text = updated;
    event = $event:mk_info(player, "Line deleted.");
    player:inform_current(event);
  endverb

  verb "write wr*ite" (any on this) owner: ARCH_WIZARD flags: "rd"
    "Write on this note";
    set_task_perms(caller_perms());
    if (!dobjstr || dobjstr == "")
      event = $event:mk_error(player, "Write what on ", $sub:i(), "?"):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    access_check = this:can_write(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_write(player, dobjstr);
  endverb

  method action_read owner: ARCH_WIZARD
    "Action handler: actor reads this note.";
    set_task_perms(this.owner);
    {who, context} = args;
    !this:can_read(who)['allowed] && return false;
    return this:do_read(who);
  endmethod

  method action_write owner: ARCH_WIZARD
    "Action handler: actor writes on this note.";
    set_task_perms(this.owner);
    {who, context, line} = args;
    !this:can_write(who)['allowed] && return false;
    return this:do_write(who, line);
  endmethod

  method action_erase owner: ARCH_WIZARD
    "Action handler: actor erases this note.";
    set_task_perms(this.owner);
    {who, context} = args;
    !this:can_write(who)['allowed] && return false;
    return this:do_erase(who);
  endmethod

  method text owner: ARCH_WIZARD
    "Return the text content if caller can read it.";
    const cp = caller_perms();
    "Owner and wizards can always read";
    if (cp == this.owner || cp.wizard)
      return this.text;
    endif
    if (this:can_read(cp)['allowed])
      return this.text;
    endif
    raise(E_PERM, "You can't read this.");
  endmethod

  method set_text owner: ARCH_WIZARD
    "Set the text content if caller can write.";
    const {new_text, ?kind = this.content_kind} = args;
    const cp = caller_perms();
    "Owner and wizards can always write";
    const can_write = cp == this.owner || cp.wizard || this:can_write(cp)['allowed];
    if (!can_write)
      raise(E_PERM, "You can't write on this.");
    endif
    if (typeof(new_text) != TYPE_LIST)
      raise(E_TYPE, "Text must be a list of strings.");
    endif
    kind in {'literal, 'djot_blocks} || raise(E_INVARG, "Notes require literal or block Djot");
    for line in (new_text)
      typeof(line) == TYPE_STR || raise(E_TYPE, "Text must be a list of strings.");
    endfor
    $format:authored(kind, new_text:join("\n"));
    this.text = new_text;
    this.content_kind = kind;
    return true;
  endmethod

  method look_self owner: ARCH_WIZARD
    "Describe writing only when the caller may read the note.";
    const actor = caller_perms();
    let description = this:description();
    if (this:can_read(actor)['allowed] && length(this:text()) > 0)
      description = $format.block:mk(description, "There appears to be some writing on it.");
    endif
    return <$look, .what = this, .title = this:name(), .description = description>;
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics this object provides.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("read", "Read a note", "Use 'read <note>' to display the note's contents.", {"r"}, 'commands, {"write", "erase"}), $help:mk("write", "Write on a note", "Use 'write <text> on <note>' to add a line of text.", {}, 'commands, {"read", "erase"}), $help:mk("erase", "Erase a note", "Use 'erase <note>' to clear all text from it.", {}, 'commands, {"read", "write"}), $help:mk("delete", "Delete a line", "Use 'delete <line#> from <note>' to remove a specific line. Use negative numbers to count from end (-1 = last line).", {"remove"}, 'commands, {"write", "erase"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
