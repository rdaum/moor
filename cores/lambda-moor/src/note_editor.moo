object NOTE_EDITOR
  name: "Note Editor"
  parent: GENERIC_EDITOR
  owner: #96
  readable: true

  property objects (owner: #96, flags: "rc") = {};
  property strmode (owner: #96, flags: "r") = {};

  override aliases = {"Note Editor", "nedit"};
  override blessed_task = 2137271057;
  override change_msg = "There are changes.";
  override commands = {{"e*dit", "<note>"}, {"save", "[<note>]"}, {"mode", "[string|list]"}};
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
    {"y*ank", "w*hat", "mode", "e*dit", "save", "abort", "q*uit,done,pause"}
  };
  override depart_msg = "A small swarm of 3x5 index cards arrives, engulfs %n, and carries %o away.";
  override entrances = {#5750};
  override help = {};
  override import_export_id = "note_editor";
  override no_change_msg = "Note has not been modified since the last save.";
  override no_littering_msg = {
    "Partially edited text will be here when you get back.",
    "To return, give the `@notedit' command with no arguments.",
    "Please come back and SAVE or ABORT if you don't intend to be working on this text in the immediate future.  Keep Our MOO Clean!  No Littering!"
  };
  override no_text_msg = "Note is devoid of text.";
  override nothing_loaded_msg = "Use the EDIT command to select a note.";
  override object_size = {9901, 1084848672};
  override previous_session_msg = "You need to ABORT or SAVE this note before editing any other.";
  override return_msg = "A small swarm of 3x5 index cards blows in and disperses, revealing %n.";
  override stateprops = {
    {"strmode", 0},
    {"objects", 0},
    {"texts", 0},
    {"changes", 0},
    {"inserting", 1},
    {"readable", 0}
  };
  override who_location_msg = "%L [editing notes]";

  verb "e*dit" (any none none) owner: #96 flags: "rd"
    if (this:changed(who = player in this.active))
      player:tell("You are still editing ", this:working_on(who), ".  Please type ABORT or SAVE first.");
    elseif (spec = this:parse_invoke(dobjstr, verb))
      this:init_session(who, @spec);
    endif
  endverb

  verb save (any none none) owner: #96 flags: "rd"
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    if (!dobjstr)
      note = this.objects[who];
    elseif (1 == (note = this:note_match_failed(dobjstr)))
      return;
    else
      this.objects[who] = note;
    endif
    text = this:text(who);
    strmode = length(text) <= 1 && this.strmode[who];
    if (strmode)
      text = text ? text[1] | "";
    endif
    if (ERR == typeof(result = this:set_note_text(note, text)))
      player:tell("Text not saved to ", this:working_on(who), ":  ", result);
      if (result == E_TYPE && typeof(note) == OBJ)
        player:tell("Do `mode list' and try saving again.");
      elseif (!dobjstr)
        player:tell("Use `save' with an argument to save the text elsewhere.");
      endif
    else
      player:tell("Text written to ", this:working_on(who), strmode ? " as a single string." | ".");
      this:set_changed(who, 0);
    endif
  endverb

  verb init_session (this none this) owner: #96 flags: "rxd"
    if (this:ok(who = args[1]))
      this.strmode[who] = strmode = typeof(text = args[3]) == STR;
      this:load(who, strmode ? text ? {text} | {} | text);
      this.objects[who] = args[2];
      player:tell("Now editing ", this:working_on(who), ".", strmode ? "  [string mode]" | "");
    endif
  endverb

  verb working_on (this none this) owner: #96 flags: "rxd"
    if (!(who = args[1]))
      return "????";
    endif
    spec = this.objects[who];
    if (typeof(spec) == LIST)
      object = spec[1];
      prop = spec[2];
    else
      object = spec;
      prop = 0;
    endif
    return valid(object) ? tostr("\"", object.name, "\"(", object, ")", prop ? "." + prop | "") | tostr(prop ? "." + prop + " on " | "", "invalid object (", object, ")");
  endverb

  verb parse_invoke (this none this) owner: #96 flags: "rxd"
    ":parse_invoke(string,verb)";
    " string is the actual commandline string indicating what we are to edit";
    " verb is the command verb that is attempting to invoke the editor";
    if (caller != this)
      raise(E_PERM);
    elseif (!(string = args[1]))
      player:tell_lines({"Usage:  " + args[2] + " <note>   (where <note> is some note object)", "        " + args[2] + "          (continues editing an unsaved note)"});
    elseif (1 == (note = this:note_match_failed(string)))
    elseif (ERR == typeof(text = this:note_text(note)))
      player:tell("Couldn't retrieve text:  ", text);
    else
      return {note, text};
    endif
    return 0;
  endverb

  verb note_text (this none this) owner: #2 flags: "rxd"
    "WIZARDLY";
    if (caller != $note_editor || caller_perms() != $note_editor.owner)
      return E_PERM;
    endif
    set_task_perms(player);
    if (typeof(spec = args[1]) == OBJ)
      text = spec:text();
    else
      text = `spec[1].((spec[2])) ! ANY';
    endif
    if ((tt = typeof(text)) in {ERR, STR} || (tt == LIST && (!text || typeof(text[1]) == STR)))
      return text;
    else
      return E_TYPE;
    endif
  endverb

  verb set_note_text (this none this) owner: #2 flags: "rxd"
    "WIZARDLY";
    if (caller != $note_editor || caller_perms() != $note_editor.owner)
      return E_PERM;
    endif
    set_task_perms(player);
    attempt = E_NONE;
    if (typeof(spec = args[1]) == OBJ)
      return spec:set_text(args[2]);
    elseif ($object_utils:has_callable_verb(spec[1], "set_" + spec[2]))
      attempt = spec[1]:(("set_" + spec[2]))(args[2]);
    endif
    if (typeof(attempt) == ERR)
      return `spec[1].((spec[2])) = args[2] ! ANY';
    else
      return attempt;
    endif
  endverb

  verb note_match_failed (this none this) owner: #96 flags: "rxd"
    if (pp = $code_utils:parse_propref(string = args[1]))
      object = pp[1];
      prop = pp[2];
    else
      object = string;
      prop = 0;
    endif
    if ($command_utils:object_match_failed(note = player:my_match_object(object, this:get_room(player)), object))
    elseif (prop)
      if (!$object_utils:has_property(note, prop))
        player:tell(object, " has no \".", prop, "\" property.");
      else
        return {note, prop};
      endif
    elseif (!$object_utils:has_callable_verb(note, "text") || !$object_utils:has_callable_verb(note, "set_text"))
      return {note, "description"};
      "... what we used to do.  but why barf?   that's no fun...";
      player:tell(object, "(", note, ") doesn't look like a note.");
    else
      return note;
    endif
    return 1;
  endverb

  verb "w*hat" (none none none) owner: #96 flags: "rd"
    pass(@args);
    if ((who = this:loaded(player)) && this.strmode[who])
      player:tell("Text will be stored as a single string instead of a list when possible.");
    endif
  endverb

  verb mode (any none none) owner: #96 flags: "rd"
    "mode [string|list]";
    if (!(who = this:loaded(player)))
      player:tell(this:nothing_loaded_msg());
      return;
    endif
    if (dobjstr && index("string", dobjstr) == 1)
      this.strmode[who] = mode = 1;
      player:tell("Now in string mode:");
    elseif (dobjstr && index("list", dobjstr) == 1)
      this.strmode[who] = mode = 0;
      player:tell("Now in list mode:");
    elseif (dobjstr)
      player:tell("Unrecognized mode:  ", dobjstr);
      player:tell("Should be one of `string' or `list'");
      return;
    else
      player:tell("Currently in ", (mode = this.strmode[who]) ? "string " | "list ", "mode:");
    endif
    if (mode)
      player:tell("  store text as a single string instead of a list when possible.");
    else
      player:tell("  always store text as a list of strings.");
    endif
  endverb

  verb local_editing_info (this none this) owner: HACKER flags: "rxd"
    {what, text} = args;
    cmd = typeof(text) == STR ? "@set-note-string" | "@set-note-text";
    name = typeof(what) == OBJ ? what.name | tostr(what[1].name, ".", what[2]);
    note = typeof(what) == OBJ ? what | tostr(what[1], ".", what[2]);
    return {name, text, tostr(cmd, " ", note)};
  endverb

  verb "set_*" (this none this) owner: #96 flags: "rxd"
    if ($perm_utils:controls(caller_perms(), this))
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb
endobject