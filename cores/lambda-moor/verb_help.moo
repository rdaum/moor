object VERB_HELP
  name: "Verb Help DB"
  parent: ROOT_CLASS
  owner: HACKER
  readable: true

  property help_msg (owner: HACKER, flags: "rc") = {
    "This is not a help database in the same way that children of $generic_help are. This object does the work when someone calls help in this way:",
    "",
    "    help <object>:<verb>",
    "",
    "It parses out the object and verb reference, pulls out the comments at the beginning of the verb, and returns them to the help system for nice display.",
    "",
    "    :find_topics(string)",
    "       tries to pull out an object:verb reference from string",
    "       returns {string} if successful",
    "       returns {} if not",
    "",
    "    :get_topic(string)",
    "       tries to pull out an object:verb reference from string (returns 0 if",
    "          it fails to do so)",
    "       tries to match the object",
    "       checks the object to see if the verb exists",
    "       pulls out the initial comments from the verb if they exist",
    "       returns a meaningful list of strings to be displayed to the player",
    "",
    "    :dump_topic(string)",
    "       does the same as :get_topic above, but returns the verb documentation",
    "          in dump form.",
    "----"
  };

  override aliases = {"verbhelp", "vh"};
  override description = "A `help database' that knows about all of the documented verbs.";
  override object_size = {3958, 1084848672};

  verb find_topics (this none this) owner: HACKER flags: "rxd"
    if ($code_utils:parse_verbref(what = args[1]))
      "... hey wow, I found it!...";
      return {what};
    else
      return {};
    endif
  endverb

  verb get_topic (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Help facility for verbs that people have bothered to document.  If the argument is a verb specification, this retrieves the code and prints any documentation lines that might be at the beginning.  Returns true if the arg can actually be interpreted as a verb specification, whether or not it is a correct one.";
    set_task_perms(caller_perms());
    if (!(spec = $code_utils:parse_verbref(args[1])))
      return 0;
    elseif ($command_utils:object_match_failed(object = $string_utils:match_object(spec[1], player.location), spec[1]))
      return 1;
    elseif (!(hv = $object_utils:has_verb(object, spec[2])))
      return "That object does not define that verb.";
    elseif (typeof(verbdoc = $code_utils:verb_documentation(object = hv[1], spec[2])) == ERR)
      return tostr(verbdoc);
    elseif (typeof(info = `verb_info(object, spec[2]) ! ANY') == ERR)
      return tostr(info);
    else
      objverb = tostr(object.name, "(", object, "):", strsub(info[3], " ", "/"));
      if (verbdoc)
        return {tostr("Information about ", objverb), "----", @verbdoc};
      else
        return tostr("No information about ", objverb);
      endif
    endif
  endverb

  verb dump_topic (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    if (!(spec = $code_utils:parse_verbref(args[1])))
      return E_INVARG;
    elseif ($command_utils:object_match_failed(object = $string_utils:match_object(spec[1], player.location), spec[1]))
      return E_INVARG;
    elseif (!(hv = $object_utils:has_verb(object, spec[2])))
      return E_VERBNF;
    elseif (typeof(vd = $code_utils:verb_documentation(hv[1], spec[2])) != LIST)
      return vd;
    else
      return {tostr(";$code_utils:set_verb_documentation(", $code_utils:corify_object(hv[1]), ",", $string_utils:print(spec[2]), ",$command_utils:read_lines())"), @$command_utils:dump_lines(vd)};
    endif
  endverb
endobject