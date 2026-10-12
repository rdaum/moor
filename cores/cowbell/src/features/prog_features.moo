object PROG_FEATURES [
  import_export_id -> "prog_features",
  import_export_hierarchy -> {"features"}
]
  name: "Programmer Features"
  parent: BUILDER_FEATURES
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Provides programmer commands (@show, @program, @grep, @chmod, @move, @which, @clear-property) for object and code management.";
  override help_source (owner: HACKER, flags: "rc") = PROG_HELP_TOPICS;

  verb eval (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <expression> -- Evaluate a MOO expression.";
    this:_challenge_command_perms();
    set_task_perms(player);
    try
      answer = eval("return " + argstr + ";", ['me -> player, 'here -> player.location], 1, 2);
      if (answer[1])
        result = answer[2];
        "Format the result";
        {result_type, result_content} = this:_format_eval_result(result);
        if (result_type == 'deflist)
          "Object result - show as definition list";
          result_event = $event:mk_eval_result(player, result_content):with_presentation_hint('inset):with_group('eval);
        else
          "Simple result - show in code block with =>";
          code = $format.code:mk("=> " + result_content, 'moo);
          result_event = $event:mk_eval_result(player, code):with_group('eval);
        endif
      else
        error_content = answer[2];
        error_text = error_content:join("\n");
        result_event = $event:mk_eval_error(player, $format.code:mk(error_text)):with_group('eval);
      endif
    except id (ANY)
      traceback = {"Eval failed: " + toliteral(id[1]) + " " + toliteral(id[2]) + ":"};
      for tb in (id[4])
        target = toliteral(tb[4]) + ":";
        if (tb[4] == #-1)
          target = "builtin function ";
        endif
        traceback = {@traceback, tostr("... called from ", target, tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6])};
      endfor
      traceback = {@traceback, "(End of traceback)"};
      result_event = $event:mk_eval_exception(player, $format.code:mk(traceback)):with_group('eval);
    endtry
    player:inform_current(result_event);
  endverb

  method _do_check_verb_exists owner: ARCH_WIZARD
    "Internal helper to check verb exists with elevated permissions";
    caller == this || raise(E_PERM);
    {verb_location, verb_name} = args;
    set_task_perms(player, {{"verb_read", verb_location, verb_name}});
    "This will raise E_VERBNF if verb doesn't exist";
    $prog_utils:get_verb_metadata(verb_location, verb_name);
    return true;
  endmethod

  verb "@edit" (any any any) owner: ARCH_WIZARD flags: "rd" [ ui -> ["label" -> "Edit"] ]
    "HINT: <object>:<verb> or <object>.<property> -- Edit a verb or property.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@edit OBJECT:VERB\n@edit OBJECT.PROPERTY")));
      return;
    endif
    target_string = argstr:trim();
    "Determine if this is a verb or property reference";
    if (":" in target_string)
      "Verb reference";
      parsed = target_string:parse_verbref();
      if (!parsed)
        player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb'"));
        return;
      endif
      {object_str, verb_name} = parsed;
      "Match the object";
      try
        target_obj = $match:match_object(object_str, player);
      except e (E_INVARG)
        player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
        return;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
        return;
      endtry
      "Find and retrieve the verb code";
      try
        verb_name = this:_do_resolve_verb_name(target_obj, verb_name);
        "Find where the verb is actually defined";
        verb_location = target_obj:find_verb_definer(verb_name);
        if (verb_location == #-1)
          player:inform_current($event:mk_error(player, "Verb '" + tostr(verb_name) + "' not found on " + target_obj.name + " or its ancestors."));
          return;
        endif
        "Check verb exists with elevated permissions";
        this:_do_check_verb_exists(verb_location, verb_name);
        "Open the editor";
        this:present_verb_editor(verb_location, verb_name);
        player:inform_current($event:mk_info(player, "Opened verb editor for " + tostr(target_obj) + ":" + tostr(verb_name)));
      except (E_VERBNF)
        player:inform_current($event:mk_error(player, "Verb '" + tostr(verb_name) + "' not found on " + target_obj.name + "."));
        return;
      endtry
    elseif ("." in target_string)
      "Property reference";
      parsed = $prog_utils:parse_target_spec(target_string);
      if (!parsed || parsed['type] != 'compound)
        player:inform_current($event:mk_error(player, "Invalid property reference format. Use 'object.property'"));
        return;
      endif
      selectors = parsed['selectors];
      if (length(selectors) != 1)
        player:inform_current($event:mk_error(player, "Invalid property reference format. Use 'object.property'"));
        return;
      endif
      selector = selectors[1];
      if (selector['kind] != 'property || !selector['item_name] || selector['inherited])
        player:inform_current($event:mk_error(player, "Invalid property reference format. Use 'object.property'"));
        return;
      endif
      object_str = parsed['object_str];
      prop_name = selector['item_name];
      "Match the object";
      try
        target_obj = $match:match_object(object_str, player);
      except e (E_INVARG)
        player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
        return;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
        return;
      endtry
      "Check property exists and open editor";
      if (!target_obj:check_property_exists(prop_name))
        player:inform_current($event:mk_error(player, "Property '" + prop_name + "' not found on " + target_obj.name + "."));
        return;
      endif
      this:present_property_editor(target_obj, prop_name);
      player:inform_current($event:mk_info(player, "Opened property editor for " + tostr(target_obj) + "." + prop_name));
    else
      player:inform_current($event:mk_error(player, "Invalid reference. Use 'object:verb' or 'object.property'"));
    endif
  endverb

  verb "@browse" (any any any) owner: ARCH_WIZARD flags: "rd" [ ui -> ["label" -> "Browse"] ]
    "HINT: <object> -- Browse an object in the object browser.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@browse OBJECT")));
      return;
    endif
    target_string = argstr:trim();
    try
      target_obj = this:_resolve_object_ref(target_string, player, "object");
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    this:present_object_browser(target_obj);
    player:inform_current($event:mk_info(player, "Opened object browser for " + tostr(target_obj)));
  endverb

  method present_verb_editor owner: ARCH_WIZARD
    caller == this || raise(E_PERM);
    {verb_location, verb_name} = args;
    editor_id = "edit-" + tostr(verb_location) + "-" + verb_name;
    editor_title = "Edit " + verb_name + " on " + tostr(verb_location);
    object_curie = $url_utils:to_curie_str(verb_location);
    present(connection(), editor_id, "text/plain", "verb-editor", "", {{"object", object_curie}, {"verb", verb_name}, {"title", editor_title}});
  endmethod

  method present_object_browser owner: ARCH_WIZARD
    caller == this || raise(E_PERM);
    {target_obj} = args;
    browser_id = "browse-" + tostr(target_obj);
    browser_title = "Browse " + tostr(target_obj);
    object_curie = $url_utils:to_curie_str(target_obj);
    present(player, browser_id, "text/plain", "object-browser", "", {{"object", object_curie}, {"title", browser_title}});
  endmethod

  method present_text_editor owner: ARCH_WIZARD
    "Present the text editor for editing freeform content.";
    "Args: {target_obj, verb_name, ?curried_args, ?initial_content, ?opts}";
    "opts is a map with optional keys: content_type, title, description, text_mode";
    "  content_type: 'text_plain (default) or 'text_djot";
    "  title: editor window title";
    "  description: explanatory blurb shown to user";
    "  text_mode: 'list (default) sends {line1, line2, ...}, 'string sends single string";
    "On save, the verb is called as: target_obj:verb_name(...curried_args, content)";
    "  where content is either a list of strings or a single string based on text_mode";
    caller == this || raise(E_PERM);
    {target_obj, verb_name, ?curried_args = {}, ?initial_content = {}, ?opts = []} = args;
    "Extract options with defaults";
    content_type = opts['content_type] || 'text_plain;
    title = opts['title] || "";
    description = opts['description] || "";
    text_mode = opts['text_mode] || 'list;
    object_curie = $url_utils:to_curie_str(target_obj);
    editor_id = "text-edit-" + tostr(target_obj) + "-" + verb_name;
    editor_title = title || "Edit text for " + tostr(target_obj);
    "Convert content to string for presentation";
    if (typeof(initial_content) == TYPE_LIST)
      content_str = initial_content:join("\n");
    else
      content_str = tostr(initial_content);
    endif
    "Convert curried args to JSON for attribute";
    args_json = toliteral(curried_args);
    "Convert content type and text mode symbols to strings";
    ct_str = content_type == 'text_djot ? "text/djot" | "text/plain";
    mode_str = text_mode == 'string ? "string" | "list";
    attrs = {{"object", object_curie}, {"verb", verb_name}, {"args", args_json}, {"content_type", ct_str}, {"title", editor_title}, {"text_mode", mode_str}};
    if (description)
      attrs = {@attrs, {"description", description}};
    endif
    present(player, editor_id, ct_str, "text-editor", content_str, attrs);
  endmethod

  method _do_get_verb_listing owner: ARCH_WIZARD
    "Internal helper to get verb listing with elevated permissions";
    caller == this || raise(E_PERM);
    {verb_location, verb_name, show_all_parens} = args;
    set_task_perms(player, {{"verb_read", verb_location, verb_name}});
    metadata = $prog_utils:get_verb_metadata(verb_location, verb_name);
    code_lines = verb_code(verb_location, verb_name, show_all_parens, true);
    return {metadata:verb_owner(), metadata:flags(), metadata:dobj(), metadata:prep(), metadata:iobj(), code_lines};
  endmethod

  method _do_get_verb_info owner: ARCH_WIZARD
    "Internal helper to get verb_info with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    set_task_perms(player, {{"verb_read", target_obj, verb_name}});
    return verb_info(target_obj, verb_name);
  endmethod

  method _do_get_verb_args owner: ARCH_WIZARD
    "Internal helper to get verb_args with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    set_task_perms(player, {{"verb_read", target_obj, verb_name}});
    return verb_args(target_obj, verb_name);
  endmethod

  verb "@list" (any any any) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["label" -> "List code", "prefix" -> "List code for"]
  ]
    "HINT: <object>:<verb> -- List verb code.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk(verb + " OBJECT:VERB [with parentheses] [without numbers]")));
      return;
    endif
    "Parse options from argstr (suffix flags only)";
    verbref_string = argstr:trim();
    show_all_parens = 0;
    hide_numbers = 0;
    while (1)
      lowered = verbref_string:lowercase();
      changed = 0;
      opt = " with parentheses";
      opt_len = length(opt);
      if (length(verbref_string) > opt_len && lowered[length(lowered) - opt_len + 1..$] == opt)
        show_all_parens = 1;
        verbref_string = verbref_string[1..length(verbref_string) - opt_len]:trim();
        changed = 1;
      endif
      opt = " without numbers";
      opt_len = length(opt);
      if (length(verbref_string) > opt_len && lowered[length(lowered) - opt_len + 1..$] == opt)
        hide_numbers = 1;
        verbref_string = verbref_string[1..length(verbref_string) - opt_len]:trim();
        changed = 1;
      endif
      if (!changed)
        break;
      endif
    endwhile
    "Parse the verb reference";
    parsed = verbref_string:parse_verbref();
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb'"));
      return;
    endif
    {object_str, verb_name} = parsed;
    "Match the object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    "Find and retrieve the verb code";
    try
      verb_name = this:_do_resolve_verb_name(target_obj, verb_name);
      verb_location = target_obj:find_verb_definer(verb_name);
      if (verb_location == #-1)
        player:inform_current($event:mk_error(player, "Verb '" + tostr(verb_name) + "' not found on " + target_obj.name + " or its ancestors."));
        return;
      endif
      "Get verb metadata and code via helper with elevated permissions";
      listing_data = this:_do_get_verb_listing(verb_location, verb_name, show_all_parens);
      {verb_owner, verb_flags, dobj, prep, iobj, code_lines} = listing_data;
      "Build metadata table";
      verb_signature = tostr(verb_location) + ":" + tostr(verb_name);
      args_spec = dobj + " " + prep + " " + iobj;
      headers = {"Verb", "Args", "Owner", "Flags"};
      row = {$format.annotation:verb(target_obj, tostr(verb_name), verb_signature, verb_location), args_spec, $format.annotation:object(verb_owner, tostr(verb_owner)), verb_flags};
      metadata_table = $format.table:mk(headers, {row});
      "Keep source exact. Renderers add a separate gutter when requested.";
      formatted_code = $format.code:mk(code_lines, 'moo, hide_numbers ? false | 1);
      "Combine table and code";
      content = $format.block:mk(metadata_table, formatted_code);
      "Create and send listing event";
      listing_event = $event:mk_program_listing(player, "", content);
      player:inform_current(listing_event);
    except (E_VERBNF)
      player:inform_current($event:mk_error(player, "Verb '" + tostr(verb_name) + "' not found on " + target_obj.name + "."));
    endtry
  endverb

  method _do_add_verb owner: ARCH_WIZARD
    "Internal helper to add verb with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_info, verb_args} = args;
    set_task_perms(player, {{"verb_add", target_obj}});
    add_verb(target_obj, verb_info, verb_args);
  endmethod

  verb "@verb" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>:<verb-names> -- Add a new verb to an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk(verb + " OBJECT:VERB-NAME(S) [DOBJ [PREP [IOBJ [PERMISSIONS [OWNER]]]]]\nHint: quote names that contain spaces or look like args.")));
      return;
    endif
    "Parse the arguments";
    colon = index(argstr, ":");
    if (!colon)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name(s)'"));
      return;
    endif
    object_token = argstr[1..colon - 1]:trim();
    remainder = argstr[colon + 1..length(argstr)]:trim();
    if (!remainder)
      player:inform_current($event:mk_error(player, "Verb name required. Use 'object:verb-name(s)'"));
      return;
    endif
    verb_tokens = remainder:words();
    if (!verb_tokens)
      player:inform_current($event:mk_error(player, "Verb name required. Use 'object:verb-name(s)'"));
      return;
    endif
    parsed = (object_token + ":" + verb_tokens[1]):parse_verbref();
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name(s)'"));
      return;
    endif
    {object_str, first_verb_name} = parsed;
    verb_tokens[1] = first_verb_name;
    "Match the object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    "Parse optional arguments with defaults";
    dobj = "none";
    prep = "none";
    iobj = "none";
    permissions = "rxd";
    verb_owner = player;
    perms_set = 0;
    cursor = length(verb_tokens);
    fn is_perm_string(s)
      if (!s)
        return false;
      endif
      for i in [1..length(s)]
        if (!(s[i] in {"r", "w", "x", "d"}))
          return false;
        endif
      endfor
      return true;
    endfn
    if (cursor >= 2)
      maybe_perms = verb_tokens[cursor - 1];
      maybe_owner = verb_tokens[cursor];
      if (is_perm_string(maybe_perms))
        try
          verb_owner = $match:match_object(maybe_owner, player);
          permissions = maybe_perms;
          perms_set = 1;
          cursor = cursor - 2;
        except e (ANY)
          "Not an owner token; leave for verb names";
        endtry
      endif
    endif
    if (!perms_set && cursor >= 1 && is_perm_string(verb_tokens[cursor]))
      permissions = verb_tokens[cursor];
      perms_set = 1;
      cursor = cursor - 1;
    endif
    if (cursor >= 1 && verb_tokens[cursor] in {"none", "this", "any"})
      iobj = verb_tokens[cursor];
      cursor = cursor - 1;
    endif
    if (cursor >= 1 && $prog_utils:is_valid_prep(verb_tokens[cursor]))
      prep = verb_tokens[cursor];
      cursor = cursor - 1;
    endif
    if (cursor >= 1 && verb_tokens[cursor] in {"none", "this", "any"})
      dobj = verb_tokens[cursor];
      cursor = cursor - 1;
    endif
    if (cursor < 1)
      player:inform_current($event:mk_error(player, "Verb name required. If names resemble args, quote the verb names."));
      return;
    endif
    verb_names = verb_tokens[1..cursor]:join(" ");
    if (!(dobj in {"none", "this", "any"}))
      player:inform_current($event:mk_error(player, "Direct object must be 'none', 'this', or 'any'"));
      return;
    endif
    if (!(iobj in {"none", "this", "any"}))
      player:inform_current($event:mk_error(player, "Indirect object must be 'none', 'this', or 'any'"));
      return;
    endif
    if (!$prog_utils:is_valid_prep(prep))
      player:inform_current($event:mk_error(player, "Invalid preposition: '" + prep + "'. Use 'none', 'any', or a valid preposition."));
      return;
    endif
    for i in [1..length(permissions)]
      char = permissions[i];
      if (!(char in {"r", "w", "x", "d"}))
        player:inform_current($event:mk_error(player, "Permissions must be subset of 'rwxd'"));
        return;
      endif
    endfor
    if (verb_owner != player && !player.wizard)
      player:inform_current($event:mk_error(player, "Only wizards can create verbs with other owners"));
      return;
    endif
    try
      this:_do_add_verb(target_obj, {verb_owner, permissions, verb_names}, {dobj, prep, iobj});
      player:inform_current($event:mk_info(player, "Verb " + tostr(target_obj) + ":" + verb_names + " added."));
    except e (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied."));
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "Unable to add verb: " + tostr(e[2])));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error adding verb: " + tostr(e[2])));
    endtry
  endverb

  method _do_delete_verb owner: ARCH_WIZARD
    "Internal helper to delete verb with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    set_task_perms(player, {{"object_write", target_obj}});
    delete_verb(target_obj, verb_name);
  endmethod

  verb "@rmverb" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>:<verb> [--dry-run] -- Remove a verb from an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, "Usage: " + verb + " object:verb-name [--dry-run]"));
      return;
    endif
    raw = argstr:trim();
    dry_run = 0;
    if (index(raw, "--dry-run"))
      dry_run = 1;
      raw = raw:replace_all(" --dry-run", "");
      raw = raw:replace_all("--dry-run", "");
      raw = raw:trim();
    endif
    spec = raw;
    parsed = $prog_utils:parse_target_spec(spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name'"));
      return;
    endif
    selectors = parsed['selectors];
    if (length(selectors) != 1)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name'"));
      return;
    endif
    selector = selectors[1];
    if (selector['kind] != 'verb || !selector['item_name] || selector['inherited])
      player:inform_current($event:mk_error(player, "Use a direct verb reference: object:verb-name"));
      return;
    endif
    object_str = parsed['object_str];
    verb_name = selector['item_name];
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    try
      verb_name = this:_do_resolve_verb_name(target_obj, verb_name);
      if (dry_run)
        player:inform_current($event:mk_info(player, "Dry-run: would remove verb " + tostr(target_obj) + ":" + verb_name));
        return;
      endif
      this:_do_delete_verb(target_obj, verb_name);
      player:inform_current($event:mk_info(player, "Verb " + tostr(target_obj) + ":" + verb_name + " removed."));
    except e (E_VERBNF)
      player:inform_current($event:mk_error(player, "Verb '" + verb_name + "' not found on " + tostr(target_obj) + "."));
    except e (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied."));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error removing verb: " + tostr(e[2])));
    endtry
  endverb

  method _do_set_verb_args owner: ARCH_WIZARD
    "Internal helper to set verb args with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_name, new_args} = args;
    set_task_perms(player, {{"verb_write", target_obj, verb_name}});
    set_verb_args(target_obj, verb_name, new_args);
  endmethod

  method _do_get_verbs owner: ARCH_WIZARD
    "Internal helper to get verb list with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return verbs(target_obj);
  endmethod

  verb "@verbs" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> -- List all verbs on an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk(verb + " OBJECT")));
      return;
    endif
    "Match the object";
    try
      target_obj = $match:match_object(argstr:trim(), player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + argstr + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + e[2]));
      return;
    endtry
    "Get verbs list with elevated permissions";
    verb_list = this:_do_get_verbs(target_obj);
    "Format as MOO literal";
    code_text = ";verbs(" + tostr(target_obj) + ") => " + toliteral(verb_list);
    formatted_code = $format.code:mk(code_text, 'moo);
    listing_event = $event:mk_eval_result(player, formatted_code);
    player:inform_current(listing_event);
  endverb

  method _do_get_properties owner: ARCH_WIZARD
    "Internal helper to get properties list with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return properties(target_obj);
  endmethod

  method _do_add_property owner: ARCH_WIZARD
    "Internal helper to add a property with elevated permissions.";
    caller == this || raise(E_PERM);
    {target_obj, prop_name, value, prop_info} = args;
    set_task_perms(player, {{"property_define", target_obj}});
    add_property(target_obj, prop_name, value, prop_info);
  endmethod

  verb "@properties @props" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object> -- List all properties on an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk(verb + " OBJECT")));
      return;
    endif
    "Match the object";
    try
      target_obj = $match:match_object(argstr:trim(), player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + argstr + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + e[2]));
      return;
    endtry
    "Get properties list with elevated permissions";
    prop_list = this:_do_get_properties(target_obj);
    "Format as MOO literal";
    code_text = ";properties(" + tostr(target_obj) + ") => " + toliteral(prop_list);
    formatted_code = $format.code:mk(code_text, 'moo);
    listing_event = $event:mk_eval_result(player, formatted_code);
    player:inform_current(listing_event);
  endverb

  verb "@prop*erty" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<property> [value [perms [owner]]] -- Add a property to an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    "Parse and validate arguments";
    !argstr && return player:inform_current($event:mk_error(player, "Usage: @property OBJECT.PROP-NAME [VALUE [PERMS [OWNER]]]"));
    parsed = $prog_utils:parse_target_spec(args[1]);
    !parsed || parsed['type] != 'compound && return player:inform_current($event:mk_error(player, "Usage: @property <object>.<prop-name>"));
    selector = parsed['selectors][1];
    selector['kind] != 'property || !selector['item_name] && return player:inform_current($event:mk_error(player, "Usage: @property <object>.<prop-name>"));
    prop_name = selector['item_name];
    target_obj = $match:match_object(parsed['object_str], player);
    "Parse optional value, perms, owner";
    value = 0;
    perms = "rc";
    owner = player;
    if (length(args) > 1)
      offset = index(argstr, args[1]) + length(args[1]);
      remainder = argstr[offset..$]:trim();
      if (remainder)
        "Try to evaluate initial value";
        eval_result = $prog_utils:eval_literal(remainder);
        if (eval_result[1])
          value = eval_result[2];
          remainder = eval_result[3]:trim();
        endif
        "Parse remaining words for perms and owner";
        remaining_words = remainder:words();
        if (length(remaining_words) > 0)
          maybe_perms = remaining_words[1];
          !match(maybe_perms, "^[rwc]*$") && return player:inform_current($event:mk_error(player, "Invalid permissions: " + maybe_perms + ". Use r, w, c (or empty for none)."));
          perms = maybe_perms;
          if (length(remaining_words) > 1)
            owner = $match:match_object(remaining_words[2], player);
          endif
        endif
      endif
    endif
    "Add the property";
    try
      this:_do_add_property(target_obj, prop_name, value, {owner, perms});
      "Format flags description";
      flag_parts = {};
      index(perms, "r") && (flag_parts = {@flag_parts, "r=read"});
      index(perms, "w") && (flag_parts = {@flag_parts, "w=write"});
      index(perms, "c") && (flag_parts = {@flag_parts, "c=chown"});
      flags_desc = flag_parts ? flag_parts:join(", ") | "(none)";
      description = tostr(target_obj) + "." + prop_name + " = " + toliteral(value);
      player:inform_current($event:mk_info(player, "Added ", $format.code:inline(description), " with flags: " + flags_desc):as_inset());
    except e (E_INVARG)
      if (index(tostr(e[2]), "Duplicate"))
        return player:inform_current($event:mk_error(player, "Property ", $format.code:inline(prop_name), " already exists on " + tostr(target_obj) + "."):as_inset());
      endif
      raise(e[1], e[2]);
    endtry
  endverb

  method _do_delete_property owner: ARCH_WIZARD
    "Internal helper to delete a property with elevated permissions.";
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    set_task_perms(player, {{"property_delete", target_obj, prop_name}});
    delete_property(target_obj, prop_name);
  endmethod

  verb "@rmprop*erty" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<property> [--dry-run] -- Remove a property from an object.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@rmproperty OBJECT.PROP-NAME [--dry-run]")));
      return;
    endif
    raw = argstr:trim();
    dry_run = 0;
    if (index(raw, "--dry-run"))
      dry_run = 1;
      raw = raw:replace_all(" --dry-run", "");
      raw = raw:replace_all("--dry-run", "");
      raw = raw:trim();
    endif
    target_spec = raw;
    parsed = $prog_utils:parse_target_spec(target_spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Usage: @rmproperty <object>.<prop-name>"));
      return;
    endif
    selector = parsed['selectors][1];
    if (selector['kind] != 'property || !selector['item_name])
      player:inform_current($event:mk_error(player, "Usage: @rmproperty <object>.<prop-name>"));
      return;
    endif
    object_str = parsed['object_str];
    prop_name = selector['item_name];
    try
      target_obj = $match:match_object(object_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
      return;
    endtry
    prop_found = 0;
    for p in (this:_do_get_properties(target_obj))
      if (tostr(p) == prop_name)
        prop_found = 1;
        break;
      endif
    endfor
    if (!prop_found)
      player:inform_current($event:mk_error(player, "Property " + prop_name + " not found on " + tostr(target_obj) + "."));
      return;
    endif
    if (dry_run)
      player:inform_current($event:mk_info(player, "Dry-run: would delete property " + tostr(target_obj) + "." + prop_name));
      return;
    endif
    try
      this:_do_delete_property(target_obj, prop_name);
      player:inform_current($event:mk_info(player, "Property " + tostr(target_obj) + "." + prop_name + " deleted."));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error deleting property: " + tostr(e[2])));
    endtry
  endverb

  verb "@args" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>:<verb> -- Show or change verb argument specifications.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk(verb + " OBJECT:VERB-NAME [DOBJ [PREP [IOBJ]]]")));
      return;
    endif
    input = argstr:trim();
    words = input:words();
    if (!words)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name'"));
      return;
    endif
    target_spec = words[1];
    parsed = target_spec:parse_verbref();
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb-name'"));
      return;
    endif
    {object_str, verb_name} = parsed;
    "Match the object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    "Get current args from metadata";
    verb_name = this:_do_resolve_verb_name(target_obj, verb_name);
    metadata = $prog_utils:get_verb_metadata(target_obj, verb_name);
    current_dobj = metadata:dobj();
    current_prep = metadata:prep();
    current_iobj = metadata:iobj();
    "If only object:verb given, show current args";
    if (length(words) == 1)
      obj_display = tostr(target_obj.name, " (", target_obj, ")");
      title = $format.title:mk(obj_display + ":" + verb_name);
      args_line = $format.code:mk(current_dobj + " " + current_prep + " " + current_iobj);
      content = $format.block:mk(title, args_line);
      player:inform_current($event:mk_info(player, content));
      return {current_dobj, current_prep, current_iobj};
    endif
    "Parse new args, using current values as defaults";
    new_dobj = length(words) >= 2 ? words[2] | current_dobj;
    new_prep = length(words) >= 3 ? words[3] | current_prep;
    new_iobj = length(words) >= 4 ? words[4] | current_iobj;
    if (!(new_dobj in {"none", "this", "any"}))
      player:inform_current($event:mk_error(player, "Direct object must be 'none', 'this', or 'any'"));
      return;
    endif
    if (!(new_iobj in {"none", "this", "any"}))
      player:inform_current($event:mk_error(player, "Indirect object must be 'none', 'this', or 'any'"));
      return;
    endif
    if (!$prog_utils:is_valid_prep(new_prep))
      player:inform_current($event:mk_error(player, "Invalid preposition: '" + new_prep + "'. Use 'none', 'any', or a valid preposition."));
      return;
    endif
    this:_do_set_verb_args(target_obj, verb_name, {new_dobj, new_prep, new_iobj});
    player:inform_current($event:mk_info(player, "Verb args updated: " + tostr(target_obj) + ":" + verb_name + " " + new_dobj + " " + new_prep + " " + new_iobj));
  endverb

  method _do_get_property_metadata owner: ARCH_WIZARD
    "Internal helper to get property metadata with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    set_task_perms(player, {{"property_read", target_obj, prop_name}});
    return $prog_utils:get_property_metadata(target_obj, prop_name);
  endmethod

  method _do_get_property_value owner: ARCH_WIZARD
    "Internal helper to get property value with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    set_task_perms(player, {{"property_read", target_obj, prop_name}});
    return target_obj.(prop_name);
  endmethod

  method _do_get_object_display_info owner: ARCH_WIZARD
    "Internal helper to get object display fields with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return ['name -> target_obj.name, 'owner -> target_obj.owner, 'parent -> parent(target_obj), 'location -> target_obj.location];
  endmethod

  method _do_find_verb_definer owner: ARCH_WIZARD
    "Internal helper to find a verb definer with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return target_obj:find_verb_definer(verb_name);
  endmethod

  method _do_get_object_match_info owner: ARCH_WIZARD
    "Internal helper to get object matching fields with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return ['name -> target_obj.name, 'aliases -> `target_obj.aliases ! ANY => {}', 'contents -> target_obj.contents, 'location -> target_obj.location];
  endmethod

  verb "@sh*ow @d*isplay" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>[selectors] -- Display object information.";
    "Syntax:";
    "  @show obj         Summary with counts and hints";
    "  @show obj.        Local properties";
    "  @show obj..       All properties (including inherited)";
    "  @show obj.name    Specific property";
    "  @show obj:        Local verbs";
    "  @show obj::       All verbs (including inherited)";
    "  @show obj:name    Specific verb";
    "  @show obj.:       Local properties and verbs";
    "  @show obj..::     All properties and verbs";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      let usage = {verb + " <object>[.<prop>|..|:<verb>|::]"};
      usage = {@usage, ""};
      usage = {@usage, "Examples:"};
      usage = {@usage, "  " + verb + " #1       Show summary with counts"};
      usage = {@usage, "  " + verb + " #1.      Show local properties"};
      usage = {@usage, "  " + verb + " #1..     Show all properties (+ inherited)"};
      usage = {@usage, "  " + verb + " #1:      Show local verbs"};
      usage = {@usage, "  " + verb + " #1::     Show all verbs (+ inherited)"};
      usage = {@usage, "  " + verb + " #1.name  Show specific property"};
      usage = {@usage, "  " + verb + " #1:tell  Show specific verb"};
      usage = {@usage, "  " + verb + " #1.:     Show local props + local verbs"};
      usage = {@usage, "  " + verb + " #1..::   Show all props + all verbs"};
      player:inform_current($event:mk_error(player, $format.code:mk(usage:join("\n"))));
      return;
    endif
    const spec = argstr:trim();
    const parsed = $prog_utils:parse_target_spec(spec);
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid specification. Use @show for usage."));
      return;
    endif
    "Match the target object";
    const object_str = parsed['object_str];
    let target_obj = $nothing;
    try
      target_obj = $match:match_object(object_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
      return;
    endtry
    "Dispatch based on type";
    if (parsed['type] == 'object)
      this:_display_summary(target_obj);
      return;
    endif
    "Compound type - always show header first, then process selectors";
    this:_display_header(target_obj);
    const selectors = parsed['selectors];
    for selector in (selectors)
      const kind = selector['kind];
      const inherited = selector['inherited];
      const item_name = selector['item_name];
      try
        if (kind == 'property)
          if (item_name)
            if (inherited)
              this:_display_inherited_property(target_obj, item_name);
            else
              this:_display_property(target_obj, item_name);
            endif
          else
            this:_display_all_properties(target_obj, inherited);
          endif
        elseif (kind == 'verb)
          if (item_name)
            if (inherited)
              this:_display_inherited_verb(target_obj, item_name);
            else
              this:_display_verb(target_obj, item_name);
            endif
          else
            this:_display_all_verbs(target_obj, inherited);
          endif
        endif
      except e (ANY)
        let message = "Error displaying selector.";
        if (typeof(e) == TYPE_LIST && length(e) >= 2 && typeof(e[2]) == TYPE_STR)
          message = message + " " + e[2];
        endif
        player:inform_current($event:mk_error(player, message));
      endtry
    endfor
  endverb

  method _display_property owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    metadata = this:_do_get_property_metadata(target_obj, prop_name);
    prop_value = metadata:is_clear() ? "(clear)" | toliteral(this:_do_get_property_value(target_obj, prop_name));
    "Truncate long values";
    if (length(prop_value) > 50)
      prop_value = prop_value[1..47] + "...";
    endif
    "Format owner as Name (#num)";
    prop_owner = metadata:owner();
    owner_str = valid(prop_owner) ? `prop_owner.name ! ANY => "???"' + " (" + tostr(prop_owner) + ")" | tostr(prop_owner);
    headers = {"Property", "Owner", "Flags", "Value"};
    row = {$format.annotation:property(target_obj, tostr(prop_name), "." + tostr(prop_name)), owner_str, metadata:perms(), prop_value};
    table = $format.table:mk(headers, {row});
    player:inform_current($event:mk_info(player, table));
  endmethod

  method _display_inherited_property owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, prop_name} = args;
    "Find where property is defined";
    current = target_obj;
    definer = #-1;
    while (valid(current))
      for p in (this:_do_get_properties(current))
        if (tostr(p) == prop_name)
          definer = current;
          break;
        endif
      endfor
      if (definer != #-1)
        break;
      endif
      current = this:_do_get_object_display_info(current)['parent];
    endwhile
    if (definer == #-1)
      raise(E_PROPNF, "Property not found: " + prop_name);
    endif
    this:_display_property(target_obj, prop_name);
  endmethod

  method _display_all_properties owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, include_inherited} = args;
    rows = {};
    if (include_inherited)
      "Include Definer column for inherited properties";
      headers = {"Property", "Definer", "Owner", "Flags", "Value"};
      seen = {};
      current = target_obj;
      while (valid(current))
        props = this:_do_get_properties(current);
        current_info = this:_do_get_object_display_info(current);
        for prop_name in (props)
          if (prop_name in seen)
            continue;
          endif
          seen = {@seen, prop_name};
          prop_display = tostr(prop_name);
          "Format definer as Name (#num)";
          definer_str = current_info['name] + " (" + tostr(current) + ")";
          try
            metadata = this:_do_get_property_metadata(current, prop_name);
          except (E_PERM)
            rows = {@rows, {$format.annotation:property(target_obj, prop_display, "." + prop_display), definer_str, "(no access)", "", ""}};
            continue;
          endtry
          prop_value = metadata:is_clear() ? "(clear)" | toliteral(`this:_do_get_property_value(current, prop_name) ! E_PERM => "(no access)"');
          if (length(prop_value) > 50)
            prop_value = prop_value[1..47] + "...";
          endif
          "Format owner as Name (#num)";
          prop_owner = metadata:owner();
          owner_str = valid(prop_owner) ? `prop_owner.name ! ANY => "???"' + " (" + tostr(prop_owner) + ")" | tostr(prop_owner);
          rows = {@rows, {$format.annotation:property(target_obj, prop_display, "." + prop_display), definer_str, owner_str, metadata:perms(), prop_value}};
        endfor
        current = current_info['parent];
      endwhile
    else
      "No Definer column for local-only";
      headers = {"Property", "Owner", "Flags", "Value"};
      props = this:_do_get_properties(target_obj);
      for prop_name in (props)
        prop_display = tostr(prop_name);
        try
          metadata = this:_do_get_property_metadata(target_obj, prop_name);
        except (E_PERM)
          rows = {@rows, {$format.annotation:property(target_obj, prop_display, "." + prop_display), "(no access)", "", ""}};
          continue;
        endtry
        prop_value = metadata:is_clear() ? "(clear)" | toliteral(`this:_do_get_property_value(target_obj, prop_name) ! E_PERM => "(no access)"');
        if (length(prop_value) > 50)
          prop_value = prop_value[1..47] + "...";
        endif
        "Format owner as Name (#num)";
        prop_owner = metadata:owner();
        owner_str = valid(prop_owner) ? `prop_owner.name ! ANY => "???"' + " (" + tostr(prop_owner) + ")" | tostr(prop_owner);
        rows = {@rows, {$format.annotation:property(target_obj, prop_display, "." + prop_display), owner_str, metadata:perms(), prop_value}};
      endfor
    endif
    if (!rows)
      player:inform_current($event:mk_info(player, "No properties found."));
      return;
    endif
    table = $format.table:mk(headers, rows);
    player:inform_current($event:mk_info(player, table));
  endmethod

  method _display_verb owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    verb_location = this:_do_find_verb_definer(target_obj, verb_name);
    if (verb_location == #-1)
      raise(E_VERBNF, "Verb not found: " + verb_name);
    endif
    verb_info_data = this:_do_get_verb_listing(verb_location, verb_name, false);
    {verb_owner, verb_flags, dobj, prep, iobj, code_lines} = verb_info_data;
    "Format owner as Name (#num)";
    owner_str = valid(verb_owner) ? `verb_owner.name ! ANY => "???"' + " (" + tostr(verb_owner) + ")" | tostr(verb_owner);
    headers = {"Verb", "Owner", "Flags", "Args"};
    args_spec = dobj + " " + prep + " " + iobj;
    verb_spec = tostr(verb_location) + ":" + verb_name;
    row = {$format.annotation:verb(target_obj, tostr(verb_name), verb_spec, verb_location), owner_str, verb_flags, args_spec};
    table = $format.table:mk(headers, {row});
    player:inform_current($event:mk_info(player, table));
  endmethod

  method _display_inherited_verb owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, verb_name} = args;
    verb_location = this:_do_find_verb_definer(target_obj, verb_name);
    if (verb_location == #-1)
      raise(E_VERBNF, "Verb not found: " + verb_name);
    endif
    this:_display_verb(target_obj, verb_name);
  endmethod

  method _display_all_verbs owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj, include_inherited} = args;
    rows = {};
    if (include_inherited)
      "Include Definer column for inherited verbs";
      headers = {"Verb", "Definer", "Owner", "Flags", "Args"};
      seen = {};
      current = target_obj;
      while (valid(current))
        verb_names = this:_do_get_verbs(current);
        current_info = this:_do_get_object_display_info(current);
        for verb_name in (verb_names)
          verb_display = tostr(verb_name);
          if (verb_name in seen)
            continue;
          endif
          seen = {@seen, verb_name};
          {verb_owner, verb_flags, dobj, prep, iobj, code_lines} = this:_do_get_verb_listing(current, verb_name, false);
          args_spec = dobj + " " + prep + " " + iobj;
          "Format definer as Name (#num)";
          definer_str = current_info['name] + " (" + tostr(current) + ")";
          "Format owner as Name (#num)";
          owner_str = valid(verb_owner) ? `verb_owner.name ! ANY => "???"' + " (" + tostr(verb_owner) + ")" | tostr(verb_owner);
          rows = {@rows, {$format.annotation:verb(target_obj, verb_display, ":" + verb_display, current), definer_str, owner_str, verb_flags, args_spec}};
        endfor
        current = current_info['parent];
      endwhile
    else
      "No Definer column for local-only";
      headers = {"Verb", "Owner", "Flags", "Args"};
      verb_names = this:_do_get_verbs(target_obj);
      for verb_name in (verb_names)
        verb_display = tostr(verb_name);
        {verb_owner, verb_flags, dobj, prep, iobj, code_lines} = this:_do_get_verb_listing(target_obj, verb_name, false);
        args_spec = dobj + " " + prep + " " + iobj;
        "Format owner as Name (#num)";
        owner_str = valid(verb_owner) ? `verb_owner.name ! ANY => "???"' + " (" + tostr(verb_owner) + ")" | tostr(verb_owner);
        rows = {@rows, {$format.annotation:verb(target_obj, verb_display, ":" + verb_display, target_obj), owner_str, verb_flags, args_spec}};
      endfor
    endif
    if (!rows)
      player:inform_current($event:mk_info(player, "No verbs found."));
      return;
    endif
    table = $format.table:mk(headers, rows);
    player:inform_current($event:mk_info(player, table));
  endmethod

  method _display_object owner: HACKER
    caller == this || raise(E_PERM);
    {target_obj} = args;
    "Show object header info as definition list, then all properties and verbs";
    obj_info = this:_do_get_object_display_info(target_obj);
    obj_name = obj_info['name];
    obj_owner = obj_info['owner];
    obj_parent = obj_info['parent];
    obj_location = obj_info['location];
    "Build deflist items";
    items = {{"Object", tostr(target_obj)}};
    items = {@items, {"Name", obj_name}};
    owner_str = valid(obj_owner) ? `obj_owner.name ! ANY => "???"' + " (" + tostr(obj_owner) + ")" | "???";
    items = {@items, {"Owner", valid(obj_owner) ? $format.annotation:object(obj_owner, owner_str) | owner_str}};
    parent_str = valid(obj_parent) ? `obj_parent.name ! ANY => "???"' + " (" + tostr(obj_parent) + ")" | "(none)";
    items = {@items, {"Parent", valid(obj_parent) ? $format.annotation:object(obj_parent, parent_str) | parent_str}};
    loc_str = valid(obj_location) ? `obj_location.name ! ANY => "???"' + " (" + tostr(obj_location) + ")" | "nowhere";
    items = {@items, {"Location", valid(obj_location) ? $format.annotation:object(obj_location, loc_str) | loc_str}};
    deflist = $format.deflist:mk(items);
    player:inform_current($event:mk_info(player, deflist));
    "Show properties (including inherited)";
    this:_display_all_properties(target_obj, true);
    "Show verbs (including inherited)";
    this:_display_all_verbs(target_obj, true);
  endmethod

  method _challenge_command_perms owner: HACKER
    player.programmer || raise(E_PERM);
  endmethod

  verb "@chmod" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <target> [<perms>] -- Show or change permissions.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@chmod OBJECT[.PROPERTY|:VERB] [PERMISSIONS]\nPERMISSIONS: rwx... | +rwx... | -rwx...")));
      return;
    endif
    "Parse target and optional permissions from the end of the command.";
    spec = argstr:trim();
    words = spec:words();
    target_spec = spec;
    perms_spec = "";
    show_only = 1;
    if (length(words) >= 2)
      maybe_perms = words[$];
      if (match(maybe_perms, "^[+-]?[rwxcd]*$"))
        perms_spec = maybe_perms;
        target_spec = words[1..$ - 1]:join(" "):trim();
        show_only = 0;
        if (!target_spec)
          player:inform_current($event:mk_error(player, $format.code:mk("@chmod OBJECT[.PROPERTY|:VERB] [PERMISSIONS]")));
          return;
        endif
      endif
    endif
    "Parse the target specification";
    parsed = $prog_utils:parse_target_spec(target_spec);
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid target reference. Use 'object', 'object.property', or 'object:verb'"));
      return;
    endif
    object_str = parsed['object_str];
    "Extract type and item_name from parsed format";
    if (parsed['type] == 'object)
      type = 'object;
      item_name = "";
    else
      selector = parsed['selectors][1];
      type = selector['kind];
      item_name = selector['item_name];
      if (!item_name)
        type = 'object;
      endif
      if (selector['inherited])
        player:inform_current($event:mk_error(player, "@chmod only works on direct object properties and verbs, not inherited ones."));
        return;
      endif
    endif
    "Match the target object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
      return;
    endtry
    fn normalize_perms(current_perms, spec_text, allowed_chars)
      op = "set";
      payload = spec_text;
      if (length(spec_text) >= 1 && (spec_text[1] == "+" || spec_text[1] == "-"))
        op = spec_text[1] == "+" ? "add" | "remove";
        payload = length(spec_text) > 1 ? spec_text[2..$] | "";
      endif
      for i in [1..length(payload)]
        ch = payload[i];
        if (!index(allowed_chars, ch))
          return {0, ch};
        endif
      endfor
      new_perms = "";
      for i in [1..length(allowed_chars)]
        ch = allowed_chars[i];
        keep = 0;
        if (op == "set")
          keep = index(payload, ch);
        elseif (op == "add")
          keep = index(current_perms, ch) || index(payload, ch);
        else
          keep = index(current_perms, ch) && !index(payload, ch);
        endif
        if (keep)
          new_perms = new_perms + ch;
        endif
      endfor
      return {1, new_perms, op};
    endfn
    "Dispatch based on type";
    if (type == 'property)
      try
        metadata = $prog_utils:get_property_metadata(target_obj, item_name);
        current_perms = metadata:perms();
        current_owner = metadata:owner();
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error getting property metadata: " + tostr(e[2])));
        return;
      endtry
      if (show_only)
        obj_display = tostr(target_obj.name, " (", target_obj, ")");
        title = $format.title:mk(obj_display + "." + item_name);
        perms_line = $format.code:mk(current_perms || "(none)");
        content = $format.block:mk(title, perms_line);
        player:inform_current($event:mk_info(player, content));
        return current_perms;
      endif
      normalized = normalize_perms(current_perms, perms_spec, "rwc");
      if (!normalized[1])
        player:inform_current($event:mk_error(player, "Property permissions must use only 'rwc'."));
        return;
      endif
      new_perms = normalized[2];
      try
        metadata:set_perms(current_owner, new_perms);
        player:inform_current($event:mk_info(player, "Property ." + item_name + " permissions set to " + (new_perms == "" ? "(cleared)" | new_perms) + " on " + tostr(target_obj) + "."));
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error setting property permissions: " + tostr(e[2])));
      endtry
    elseif (type == 'verb)
      try
        metadata = $prog_utils:get_verb_metadata(target_obj, item_name);
        current_perms = metadata:flags();
        current_owner = metadata:verb_owner();
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error getting verb metadata: " + tostr(e[2])));
        return;
      endtry
      if (show_only)
        obj_display = tostr(target_obj.name, " (", target_obj, ")");
        title = $format.title:mk(obj_display + ":" + item_name);
        perms_line = $format.code:mk(current_perms || "(none)");
        content = $format.block:mk(title, perms_line);
        player:inform_current($event:mk_info(player, content));
        return current_perms;
      endif
      normalized = normalize_perms(current_perms, perms_spec, "rwxd");
      if (!normalized[1])
        player:inform_current($event:mk_error(player, "Verb permissions must use only 'rwxd'."));
        return;
      endif
      new_perms = normalized[2];
      try
        metadata:set_perms(current_owner, new_perms);
        player:inform_current($event:mk_info(player, "Verb :" + item_name + " permissions set to " + (new_perms == "" ? "(cleared)" | new_perms) + " on " + tostr(target_obj) + "."));
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error setting verb permissions: " + tostr(e[2])));
      endtry
    elseif (type == 'object)
      current_flags = target_obj.f;
      current_perms = "";
      if (current_flags % 2 == 1)
        current_perms = current_perms + "r";
      endif
      if (current_flags / 2 % 2 == 1)
        current_perms = current_perms + "w";
      endif
      if (show_only)
        obj_display = tostr(target_obj.name, " (", target_obj, ")");
        title = $format.title:mk(obj_display);
        perms_line = $format.code:mk(current_perms || "(none)");
        content = $format.block:mk(title, perms_line);
        player:inform_current($event:mk_info(player, content));
        return current_perms;
      endif
      normalized = normalize_perms(current_perms, perms_spec, "rw");
      if (!normalized[1])
        player:inform_current($event:mk_error(player, "Object permissions must use only 'rw'."));
        return;
      endif
      new_perms = normalized[2];
      flags = 0;
      if (index(new_perms, "r"))
        flags = flags + 1;
      endif
      if (index(new_perms, "w"))
        flags = flags + 2;
      endif
      target_obj.f = flags;
      player:inform_current($event:mk_info(player, "Object " + tostr(target_obj) + " permissions set to " + (new_perms == "" ? "(cleared)" | new_perms) + "."));
    else
      player:inform_current($event:mk_error(player, "Invalid reference type"));
    endif
  endverb

  method _do_grep_object owner: HACKER
    "Search one object's verbs for @grep and report inaccessible verb skips.";
    "Returns a map with matches and skipped_verb_count.";
    caller == this || raise(E_PERM);
    {pattern, search_obj, casematters, use_regex, owner_filter} = args;
    matches = {};
    skipped_verb_count = 0;
    verb_list = this:_do_get_verbs(search_obj);
    pattern_cmp = casematters ? pattern | pattern:lowercase();
    for vname in (verb_list)
      try
        listing = this:_do_get_verb_listing(search_obj, vname, 1);
      except (E_PERM)
        skipped_verb_count = skipped_verb_count + 1;
        continue;
      endtry
      {vowner, verb_flags, dobj, prep, iobj, lines} = listing;
      if (owner_filter != 0 && vowner != owner_filter)
        continue;
      endif
      for line_num in [1..length(lines)]
        line = lines[line_num];
        hay = casematters ? line | line:lowercase();
        if (use_regex)
          ok = match(hay, pattern_cmp);
          if (!ok)
            continue;
          endif
          matching_line = line;
        else
          match_pos = index(hay, pattern_cmp, 1);
          if (!match_pos)
            continue;
          endif
          max_len = 50;
          line_len = length(line);
          if (line_len <= max_len)
            matching_line = line;
          else
            pattern_len = length(pattern);
            start_pos = max(1, match_pos - (max_len - pattern_len) / 2);
            end_pos = min(line_len, start_pos + max_len - 1);
            if (end_pos == line_len && end_pos - start_pos < max_len - 1)
              start_pos = max(1, end_pos - max_len + 1);
            endif
            matching_line = line[start_pos..end_pos];
            if (start_pos > 1)
              matching_line = "..." + matching_line;
            endif
            if (end_pos < line_len)
              matching_line = matching_line + "...";
            endif
          endif
        endif
        owner_name = valid(vowner) ? `this:_do_get_object_display_info(vowner)['name] ! ANY => tostr(vowner)' | "Recycled";
        matches = {@matches, {search_obj, vname, owner_name, tostr(vowner), line_num, matching_line}};
      endfor
    endfor
    return ['matches -> matches, 'skipped_verb_count -> skipped_verb_count];
  endmethod

  method _do_get_available_objects owner: ARCH_WIZARD
    "Internal helper to get list of objects available for searching";
    caller == this || raise(E_PERM);
    perms = caller_perms();
    set_task_perms(player, {{"object_list"}});
    all_objects = objects();
    if (perms.wizard)
      return all_objects;
    endif
    available = {};
    for o in (all_objects)
      info = this:_do_get_object_visibility_info(o);
      if (info['owner] == perms || info['readable])
        available = {@available, o};
      endif
    endfor
    return available;
  endmethod

  method _do_get_object_visibility_info owner: ARCH_WIZARD
    "Internal helper to get object visibility fields with elevated permissions";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    set_task_perms(player, {{"object_read", target_obj}});
    return ['owner -> target_obj.owner, 'readable -> target_obj.r];
  endmethod

  verb "@grep" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: [options] <pattern> [<object>] -- Search verb code for a pattern.";
    this:_challenge_command_perms();
    set_task_perms(player);
    start_time = ftime();
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@grep [--regex] [--case-sensitive|-s|-i] [--owner OBJECT] [--limit N] PATTERN [OBJECT]")));
      return;
    endif
    input = argstr:trim();
    if (!input)
      player:inform_current($event:mk_error(player, $format.code:mk("@grep [options] PATTERN [OBJECT]")));
      return;
    endif
    tokens = input:words();
    idx = 1;
    casematters = 0;
    use_regex = 0;
    owner_filter = 0;
    limit = 0;
    while (idx <= length(tokens))
      tok = tokens[idx];
      if (tok == "-i")
        casematters = 0;
        idx = idx + 1;
        continue;
      elseif (tok == "-s" || tok == "--case-sensitive")
        casematters = 1;
        idx = idx + 1;
        continue;
      elseif (tok == "-r" || tok == "--regex")
        use_regex = 1;
        idx = idx + 1;
        continue;
      elseif (tok == "--owner")
        if (idx == length(tokens))
          player:inform_current($event:mk_error(player, "--owner requires an object reference."));
          return;
        endif
        owner_tok = tokens[idx + 1];
        try
          owner_filter = $match:match_object(owner_tok, player);
        except e (ANY)
          player:inform_current($event:mk_error(player, "Could not resolve owner: " + tostr(e[2])));
          return;
        endtry
        idx = idx + 2;
        continue;
      elseif (tok == "--limit")
        if (idx == length(tokens))
          player:inform_current($event:mk_error(player, "--limit requires a positive integer."));
          return;
        endif
        limit_tok = tokens[idx + 1];
        try
          limit = toint(limit_tok);
        except e (E_INVARG)
          player:inform_current($event:mk_error(player, "--limit requires a positive integer."));
          return;
        except e (ANY)
          player:inform_current($event:mk_error(player, "Error parsing --limit: " + tostr(e[2])));
          return;
        endtry
        if (limit <= 0)
          player:inform_current($event:mk_error(player, "--limit requires a positive integer."));
          return;
        endif
        idx = idx + 2;
        continue;
      endif
      break;
    endwhile
    if (idx > length(tokens))
      player:inform_current($event:mk_error(player, "Missing pattern."));
      return;
    endif
    pattern = tokens[idx..$]:join(" "):trim();
    if (use_regex)
      try
        match("", casematters ? pattern | pattern:lowercase());
      except e (ANY)
        player:inform_current($event:mk_error(player, "Invalid regular expression: " + tostr(e[2])));
        return;
      endtry
    endif
    search_objects = this:_do_get_available_objects();
    if (length(tokens[idx..$]) >= 2)
      possible_target = tokens[$];
      try
        target_obj = $match:match_object(possible_target, player);
        pattern = tokens[idx..$ - 1]:join(" "):trim();
        if (!pattern)
          player:inform_current($event:mk_error(player, "Missing pattern before target object."));
          return;
        endif
        search_objects = {target_obj};
      except e (E_INVARG, E_PROPNF)
        "No explicit target object; treat full remainder as pattern.";
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error resolving grep target: " + toliteral(e)));
        return;
      endtry
    endif
    player:inform_current($event:mk_info(player, "Searching for \"" + pattern + "\" in " + tostr(length(search_objects)) + " objects..."));
    all_matches = {};
    obj_count = 0;
    skipped_verb_count = 0;
    for o in (search_objects)
      obj_count = obj_count + 1;
      if (obj_count % 3 == 0)
        suspend_if_needed();
      endif
      try
        grep_result = this:_do_grep_object(pattern, o, casematters, use_regex, owner_filter);
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error searching " + tostr(o) + ": " + toliteral(e)));
        return;
      endtry
      all_matches = {@all_matches, @grep_result['matches]};
      skipped_verb_count = skipped_verb_count + grep_result['skipped_verb_count];
      if (limit && length(all_matches) >= limit)
        all_matches = all_matches[1..limit];
        break;
      endif
    endfor
    if (!all_matches)
      elapsed = ftime() - start_time;
      no_match_msg = "No matches found.";
      if (skipped_verb_count)
        no_match_msg = no_match_msg + " Skipped " + tostr(skipped_verb_count) + " inaccessible verb" + (skipped_verb_count == 1 ? "" | "s") + ".";
      endif
      player:inform_current($event:mk_info(player, no_match_msg));
      player:inform_current($event:mk_info(player, tostr("Time: ", elapsed, "s")));
      return;
    endif
    headers = {"Verb", "Line", "Owner", "Code"};
    rows = {};
    row_count = 0;
    for match in (all_matches)
      row_count = row_count + 1;
      if (row_count % 5 == 0)
        suspend_if_needed();
      endif
      {o, verb_name, owner_name, owner_id, line_num, code_snippet} = match;
      verb_ref = toliteral(o) + ":" + verb_name;
      owner_str = owner_name + " (" + owner_id + ")";
      code_cell = $format.code:mk(code_snippet, 'moo);
      rows = {@rows, {verb_ref, tostr(line_num), owner_str, code_cell}};
    endfor
    suspend_if_needed();
    result_table = $format.table:mk(headers, rows);
    summary = tostr("Found ", length(all_matches), " match", length(all_matches) != 1 ? "es" | "", " for \"", pattern, "\"");
    if (owner_filter != 0)
      summary = summary + " owner=" + tostr(owner_filter);
    endif
    if (limit)
      summary = summary + " (limit " + tostr(limit) + ")";
    endif
    if (skipped_verb_count)
      summary = summary + "; skipped " + tostr(skipped_verb_count) + " inaccessible verb" + (skipped_verb_count == 1 ? "" | "s");
    endif
    content = $format.block:mk(summary, result_table);
    elapsed = ftime() - start_time;
    player:inform_current($event:mk_info(player, content));
    player:inform_current($event:mk_info(player, tostr("Time: ", elapsed, "s")));
  endverb

  verb "@codep*aste" (none none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: -- Paste MOO code with syntax highlighting to the room.";
    this:_challenge_command_perms();
    if (!valid(player.location))
      return;
    endif
    content = player:read_multiline("Enter MOO code to paste");
    if (content == "@abort" || typeof(content) != TYPE_STR)
      player:inform_current($event:mk_info(player, "Code paste aborted."));
      return;
    endif
    lines = content:split("\n");
    if (length(lines) > 50)
      player:inform_current($event:mk_error(player, "Code paste is greater than 50 lines, too long."));
      return;
    endif
    title = $format.title:mk({$sub:nc(), " ", $sub:self_alt("codepaste", "codepastes")}, 4);
    code = $format.code:mk(content, 'moo);
    event = $event:mk_paste(player, title, code):with_presentation_hint('inset):with_group('paste);
    player.location:announce(event);
  endverb

  verb "@doc*umentation" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object|builtin> -- Display developer documentation.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@doc OBJECT\n@doc OBJECT:VERB\n@doc OBJECT.PROPERTY\n@doc BUILTIN_FUNCTION")));
      return;
    endif
    target_spec = argstr:trim();
    "Check if this might be a builtin function name";
    if (index(target_spec, ":") == 0 && index(target_spec, ".") == 0 && index(target_spec, "#") == 0 && index(target_spec, "$") == 0)
      try
        func_name = tosym(target_spec);
        doc_lines = function_help(func_name);
        title = {"Builtin Function: ", $format.code:inline(target_spec)};
        doc_text = doc_lines:join("\n");
        for fn_info in (function_info())
          if (fn_info[1] == target_spec)
            min_args = fn_info[2];
            max_args = fn_info[3];
            sig_info = "Arguments: " + tostr(min_args) + (max_args == -1 ? "+" | max_args == min_args ? "" | "-" + tostr(max_args));
            doc_text = sig_info + "\n\n" + doc_text;
            break;
          endif
        endfor
        content = $help_utils:format_documentation_display(title, $format.code:mk(doc_text));
        player:inform_current($event:mk_info(player, content):as_inset());
        return;
      except e (E_INVARG)
        "Not a builtin function - continue to object parsing.";
      endtry
    endif
    parsed = $prog_utils:parse_target_spec(target_spec);
    if (!parsed)
      if (!this:suggest_doc_topic('builtin, target_spec))
        player:inform_current($event:mk_error(player, "Invalid format. Use ",
          $format.code:inline("object"), ", ", $format.code:inline("object:verb"), ", ",
          $format.code:inline("object.property"), ", or ", $format.code:inline("builtin_function")));
      endif
      return;
    endif
    object_str = parsed['object_str];
    if (parsed['type] == 'object)
      type = 'object;
      item_name = "";
    else
      selectors = parsed['selectors];
      if (length(selectors) != 1)
        player:inform_current($event:mk_error(player, "Use exactly one selector: object:verb or object.property"));
        return;
      endif
      selector = selectors[1];
      type = selector['kind];
      item_name = selector['item_name];
      if (!item_name)
        player:inform_current($event:mk_error(player, "Use object:verb or object.property (selector name required)."));
        return;
      endif
    endif
    "Match the target object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (ANY)
      if (!this:suggest_doc_topic('object, target_spec))
        player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
      endif
      return;
    endtry
    "Dispatch based on type";
    if (type == 'object)
      obj_display = `target_obj:display_name() ! E_VERBNF => target_obj.name';
      doc_text = $help_utils:get_object_documentation(target_obj);
      title = {"Documentation for ", $format.annotation:object(target_obj, obj_display + " (" + toliteral(target_obj) + ")")};
      content = $help_utils:format_documentation_display(title, doc_text);
      player:inform_current($event:mk_info(player, content):as_inset());
    elseif (type == 'verb)
      verb_location = target_obj:find_verb_definer(item_name);
      if (verb_location == #-1)
        if (!this:suggest_doc_topic('verb, target_spec, target_obj, item_name))
          player:inform_current($event:mk_error(player, "Verb ", $format.code:inline(tostr(item_name)), " not found on ", $format.annotation:object(target_obj), " or its ancestors."));
        endif
        return;
      endif
      verb_obj_display = `verb_location:display_name() ! E_VERBNF => verb_location.name';
      doc_text = $help_utils:extract_verb_documentation(verb_location, item_name);
      title = {"Documentation for " + verb_obj_display + " (", $format.annotation:verb(verb_location, item_name), ")"};
      content = $help_utils:format_documentation_display(title, $format.code:mk(doc_text));
      player:inform_current($event:mk_info(player, content):as_inset());
    elseif (type == 'property)
      if (!(item_name in properties(target_obj)))
        if (!this:suggest_doc_topic('property, target_spec, target_obj, item_name))
          player:inform_current($event:mk_error(player, "Property ", $format.code:inline(item_name), " not found on ", $format.annotation:object(target_obj), "."));
        endif
        return;
      endif
      obj_display = `target_obj:display_name() ! E_VERBNF => target_obj.name';
      doc_text = $help_utils:property_documentation(target_obj, item_name);
      title = {"Documentation for " + obj_display + " (", $format.annotation:property(target_obj, item_name), ")"};
      content = $help_utils:format_documentation_display(title, $format.code:mk(doc_text));
      player:inform_current($event:mk_info(player, content):as_inset());
    else
      player:inform_current($event:mk_error(player, "Invalid reference type"));
    endif
  endverb

  verb "@rename" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Rename an object or verb. Usage: @rename <object> to <name[:aliases]> or @rename <object>:<verb> to <new-verb-name>";
    caller != player && raise(E_PERM);
    player.programmer || raise(E_PERM, "Programmer features required.");
    set_task_perms(player);
    "Check for verb rename syntax (colon in dobjstr)";
    if (dobjstr && ":" in dobjstr)
      "Verb rename - validate args";
      if (!argstr || (prepstr && prepstr != "to") || !iobjstr)
        player:inform_current($event:mk_error(player, $format.code:mk("@rename OBJECT:VERB to NEW-VERB-NAME")));
        return;
      endif
      try
        "Parse object:verb";
        parsed = dobjstr:parse_verbref();
        if (!parsed)
          player:inform_current($event:mk_error(player, "Invalid verb reference format. Use 'object:verb'"));
          return;
        endif
        {object_str, verb_name} = parsed;
        "Match the object";
        target_obj = $match:match_object(object_str, player);
        typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
        !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
        "Check verb exists";
        verb_name = this:_do_resolve_verb_name(target_obj, verb_name);
        info = `verb_info(target_obj, verb_name) ! E_VERBNF => 0';
        if (!info)
          player:inform_current($event:mk_error(player, "Verb '" + verb_name + "' not found on " + tostr(target_obj) + "."));
          return;
        endif
        {verb_owner, verb_perms, old_names} = info;
        "Check permissions - must be owner or wizard";
        if (!player.wizard && verb_owner != player)
          raise(E_PERM, "You do not have permission to rename " + tostr(target_obj) + ":" + verb_name + ".");
        endif
        "Get new verb name(s) - spaces separate multiple names";
        new_names = iobjstr:trim();
        !new_names && raise(E_INVARG, "Verb name cannot be blank.");
        "Set the new verb name";
        set_verb_info(target_obj, verb_name, {verb_owner, verb_perms, new_names});
        message = "Renamed verb " + tostr(target_obj) + ":" + verb_name + " to \"" + new_names + "\".";
        player:inform_current($event:mk_info(player, message));
        return 1;
      except e (ANY)
        message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
        player:inform_current($event:mk_error(player, message));
        return 0;
      endtry
    endif
    "Object rename - check for valid usage";
    if (!argstr || !dobjstr || (prepstr && prepstr != "to") || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@rename OBJECT to NAME[:ALIASES]\n@rename OBJECT:VERB to NEW-VERB-NAME")));
      return;
    endif
    try
      target_obj = $match:match_object(dobjstr, player);
      typeof(target_obj) != TYPE_OBJ && raise(E_INVARG, "That reference is not an object.");
      !valid(target_obj) && raise(E_INVARG, "That object no longer exists.");
      if (!player.wizard && target_obj.owner != player)
        raise(E_PERM, "You do not have permission to rename " + tostr(target_obj) + ".");
      endif
      parsed = $str_proto:parse_name_aliases(iobjstr);
      new_name = parsed[1];
      new_aliases = parsed[2];
      !new_name && raise(E_INVARG, "Object name cannot be blank.");
      old_name = `target_obj.name ! ANY => "(no name)"';
      this:_do_rename_object(target_obj, new_name, new_aliases);
      message = "Renamed \"" + old_name + "\" (" + tostr(target_obj) + ") to \"" + new_name + "\".";
      if (new_aliases)
        alias_str = new_aliases:join(", ");
        message = message + " Aliases: " + alias_str + ".";
      endif
      player:inform_current($event:mk_info(player, message));
      return 1;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return 0;
    endtry
  endverb

  method help_topics owner: ARCH_WIZARD
    "Return help topics for programmer commands.";
    {for_player, ?topic = ""} = args;
    source = this.help_source;
    if (valid(source) && respond_to(source, 'help_topics))
      source_result = source:help_topics(for_player, topic);
      if (typeof(source_result) != TYPE_INT)
        return source_result;
      endif
    endif
    if (topic != "")
      verb_help = $help_utils:verb_help_from_hint(this, topic, 'programming);
      typeof(verb_help) != TYPE_INT && return verb_help;
      return 0;
    endif
    return {};
  endmethod

  method _format_eval_result owner: HACKER
    "Format an eval result. Returns {type, content} where type is 'simple or 'deflist.";
    "For simple values, content is a string. For objects, content is a deflist flyweight.";
    {result} = args;
    if (typeof(result) == TYPE_OBJ)
      "Format object as definition list";
      obj_str = tostr(result);
      if (valid(result))
        obj_info = this:_do_get_object_display_info(result);
        name = obj_info['name];
        owner = obj_info['owner];
        owner_str = valid(owner) ? `this:_do_get_object_display_info(owner)['name] ! ANY => tostr(owner)' + " (" + tostr(owner) + ")" | "???";
        loc = obj_info['location];
        loc_str = valid(loc) ? `this:_do_get_object_display_info(loc)['name] ! ANY => tostr(loc)' + " (" + tostr(loc) + ")" | "nowhere";
        items = {{"Object", $format.annotation:object(result, obj_str)}, {"Name", name}, {"Owner", valid(owner) ? $format.annotation:object(owner, owner_str) | owner_str}, {"Location", valid(loc) ? $format.annotation:object(loc, loc_str) | loc_str}};
        return {'deflist, $format.deflist:mk(items)};
      else
        return {'simple, obj_str + " (invalid)"};
      endif
    endif
    "For non-objects, just use toliteral";
    return {'simple, toliteral(result)};
  endmethod

  method _display_summary owner: HACKER
    "Display object summary with counts and usage hints.";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    "Get object info";
    obj_info = this:_do_get_object_display_info(target_obj);
    obj_name = obj_info['name];
    obj_owner = obj_info['owner];
    obj_parent = obj_info['parent];
    obj_location = obj_info['location];
    "Count local properties and verbs";
    local_props = this:_do_get_properties(target_obj);
    local_verbs = this:_do_get_verbs(target_obj);
    local_prop_count = length(local_props);
    local_verb_count = length(local_verbs);
    "Count inherited (walk up parent chain)";
    inherited_prop_count = 0;
    inherited_verb_count = 0;
    current = obj_parent;
    while (valid(current))
      inherited_prop_count = inherited_prop_count + length(this:_do_get_properties(current));
      inherited_verb_count = inherited_verb_count + length(this:_do_get_verbs(current));
      current = this:_do_get_object_display_info(current)['parent];
    endwhile
    "Capture object labels as literal reference content";
    obj_ref = tostr(target_obj);
    items = {{"Object", $format.annotation:object(target_obj, obj_ref)}};
    items = {@items, {"Name", obj_name}};
    owner_str = valid(obj_owner) ? `obj_owner.name ! ANY => "???"' + " (" + tostr(obj_owner) + ")" | "???";
    items = {@items, {"Owner", valid(obj_owner) ? $format.annotation:object(obj_owner, owner_str) | owner_str}};
    parent_str = valid(obj_parent) ? `obj_parent.name ! ANY => "???"' + " (" + tostr(obj_parent) + ")" | "(none)";
    items = {@items, {"Parent", valid(obj_parent) ? $format.annotation:object(obj_parent, parent_str) | parent_str}};
    loc_str = valid(obj_location) ? `obj_location.name ! ANY => "???"' + " (" + tostr(obj_location) + ")" | "nowhere";
    items = {@items, {"Location", valid(obj_location) ? $format.annotation:object(obj_location, loc_str) | loc_str}};
    "Add counts to same deflist";
    prop_summary = tostr(local_prop_count) + " local";
    if (inherited_prop_count > 0)
      prop_summary = prop_summary + ", " + tostr(inherited_prop_count) + " inherited";
    endif
    verb_summary = tostr(local_verb_count) + " local";
    if (inherited_verb_count > 0)
      verb_summary = verb_summary + ", " + tostr(inherited_verb_count) + " inherited";
    endif
    items = {@items, {"Properties", prop_summary}};
    items = {@items, {"Verbs", verb_summary}};
    deflist = $format.deflist:mk(items);
    hint = $format.paragraph:mk("Try: ", $format.code:inline("@show " + obj_ref + ".:"),
      " (local) or ", $format.code:inline("@show " + obj_ref + "..::"), " (all). See ",
      $format.code:inline("@show"), " for syntax.");
    content = $format.block:mk(deflist, hint);
    event = $event:mk_info(player, content);
    player:inform_current(event);
  endmethod

  method _display_header owner: HACKER
    "Display object header info only (no counts/hints).";
    caller == this || raise(E_PERM);
    {target_obj} = args;
    "Get object info";
    obj_info = this:_do_get_object_display_info(target_obj);
    obj_name = obj_info['name];
    obj_owner = obj_info['owner];
    obj_parent = obj_info['parent];
    obj_location = obj_info['location];
    "Capture object labels as literal reference content";
    obj_ref = tostr(target_obj);
    items = {{"Object", $format.annotation:object(target_obj, obj_ref)}};
    items = {@items, {"Name", obj_name}};
    owner_str = valid(obj_owner) ? `obj_owner.name ! ANY => "???"' + " (" + tostr(obj_owner) + ")" | "???";
    items = {@items, {"Owner", valid(obj_owner) ? $format.annotation:object(obj_owner, owner_str) | owner_str}};
    parent_str = valid(obj_parent) ? `obj_parent.name ! ANY => "???"' + " (" + tostr(obj_parent) + ")" | "(none)";
    items = {@items, {"Parent", valid(obj_parent) ? $format.annotation:object(obj_parent, parent_str) | parent_str}};
    loc_str = valid(obj_location) ? `obj_location.name ! ANY => "???"' + " (" + tostr(obj_location) + ")" | "nowhere";
    items = {@items, {"Location", valid(obj_location) ? $format.annotation:object(obj_location, loc_str) | loc_str}};
    deflist = $format.deflist:mk(items);
    event = $event:mk_info(player, deflist);
    player:inform_current(event);
  endmethod

  verb suggest_doc_topic (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Suggest @doc targets when lookup fails, using LLM.";
    "Args: failure_type ('object, 'verb, 'property, 'builtin), target_spec, ?target_obj, ?item_name";
    "Returns true if handled (placeholder sent), false if LLM not available.";
    {failure_type, target_spec, ?target_obj = #-1, ?item_name = ""} = args;
    "Check if LLM is available";
    llm_client = $player.suggestions_llm_client;
    if (typeof(llm_client) != TYPE_OBJ || !valid(llm_client))
      return false;
    endif
    "Capture current connection BEFORE forking";
    all_conns = connections();
    if (!all_conns || length(all_conns) == 0)
      return false;
    endif
    current_conn = all_conns[1][1];
    "Get display name for target_obj - find sysref name (e.g. #13 -> '$str_proto')";
    obj_display = target_spec;
    if (valid(target_obj))
      obj_display = tostr(target_obj);
      for prop in (properties(#0))
        val = #0.(prop);
        if (val == target_obj)
          obj_display = "$" + prop;
          break;
        endif
      endfor
    endif
    "Build error message based on failure type";
    if (failure_type == 'object)
      error_msg = "Could not find object: " + target_spec;
    elseif (failure_type == 'verb)
      error_msg = "Verb " + item_name + " not found on " + obj_display + ".";
    elseif (failure_type == 'builtin)
      error_msg = target_spec + " is not a recognized builtin function or object.";
    else
      error_msg = "Property " + item_name + " not found on " + obj_display + ".";
    endif
    "Send immediate placeholder with rewritable event";
    rewrite_id = uuid();
    placeholder = $event:mk_error(player, error_msg + " (Finding suggestions...)"):with_rewritable(rewrite_id, 30, error_msg):with_presentation_hint('processing):with_audience('utility);
    player:inform_current(placeholder);
    "Fork the LLM query so we return immediately";
    fork (0)
      "Build context based on failure type";
      prompt = "You help programmers find documentation in a MOO (text-based virtual world). ";
      prompt = prompt + "A programmer tried '@doc " + target_spec + "' but it failed.\n\n";
      if (failure_type == 'builtin)
        "Builtin function not found - provide list of builtins";
        prompt = prompt + "They tried to look up a builtin function.\n\n";
        prompt = prompt + "AVAILABLE BUILTIN FUNCTIONS:\n";
        builtin_names = {};
        for fn_info in (function_info())
          builtin_names = {@builtin_names, fn_info[1]};
        endfor
        prompt = prompt + builtin_names:join(", ") + "\n\n";
      elseif (failure_type == 'object)
        "Object not found - provide list of sysref objects if they used $ prefix";
        if (target_spec[1] == "$")
          prompt = prompt + "They tried to look up a system object starting with '$'.\n\n";
          prompt = prompt + "AVAILABLE SYSTEM OBJECTS ($name format):\n";
          "Get all properties on #0 that point to valid objects";
          sysrefs = {};
          for prop in (properties(#0))
            val = #0.(prop);
            if (typeof(val) == TYPE_OBJ && valid(val))
              sysrefs = {@sysrefs, "$" + prop};
            endif
          endfor
          prompt = prompt + sysrefs:join(", ") + "\n\n";
        else
          prompt = prompt + "They tried to look up an object but it wasn't found.\n";
          prompt = prompt + "Suggest they use an object number (#123) or system object ($name).\n";
          prompt = prompt + "They might also have meant a builtin function.\n\n";
          prompt = prompt + "AVAILABLE BUILTIN FUNCTIONS:\n";
          builtin_names = {};
          for fn_info in (function_info())
            builtin_names = {@builtin_names, fn_info[1]};
          endfor
          prompt = prompt + builtin_names:join(", ") + "\n\n";
        endif
      elseif (failure_type == 'verb)
        "Verb not found - provide list of verbs on the object";
        prompt = prompt + "They tried to look up verb '" + item_name + "' on " + obj_display + ".\n\n";
        prompt = prompt + "VERBS ON " + obj_display + ":\n";
        verb_list = verbs(target_obj);
        if (length(verb_list) > 50)
          verb_list = verb_list[1..50];
          prompt = prompt + verb_list:join(", ") + " ... (and more)\n\n";
        else
          prompt = prompt + verb_list:join(", ") + "\n\n";
        endif
        "Also check ancestors for inherited verbs";
        inherited = {};
        for anc in (ancestors(target_obj))
          for v in (verbs(anc))
            if (!(v in verb_list) && !(v in inherited) && length(inherited) < 20)
              inherited = {@inherited, v};
            endif
          endfor
        endfor
        if (length(inherited) > 0)
          prompt = prompt + "INHERITED VERBS (from ancestors):\n";
          prompt = prompt + inherited:join(", ") + "\n\n";
        endif
      else
        "Property not found - provide list of properties on the object";
        prompt = prompt + "They tried to look up property '" + item_name + "' on " + obj_display + ".\n\n";
        prompt = prompt + "PROPERTIES ON " + obj_display + ":\n";
        prop_list = properties(target_obj);
        if (length(prop_list) > 50)
          prop_list = prop_list[1..50];
          prompt = prompt + prop_list:join(", ") + " ... (and more)\n\n";
        else
          prompt = prompt + prop_list:join(", ") + "\n\n";
        endif
      endif
      prompt = prompt + "INSTRUCTIONS:\n";
      prompt = prompt + "1. Suggest 1-3 likely matches based on what they typed\n";
      if (failure_type == 'verb)
        prompt = prompt + "2. Format suggestions as '@doc " + obj_display + ":VERBNAME'\n";
      elseif (failure_type == 'property)
        prompt = prompt + "2. Format suggestions as '@doc " + obj_display + ".PROPNAME'\n";
      elseif (failure_type == 'builtin)
        prompt = prompt + "2. Format suggestions as '@doc FUNCTION_NAME'\n";
      else
        prompt = prompt + "2. Format suggestions as '@doc <target>'\n";
      endif
      prompt = prompt + "3. Keep response under 60 words\n";
      prompt = prompt + "4. Use plain text without markup\n";
      "Call LLM and rewrite the placeholder";
      try
        response = llm_client:simple_query(prompt);
        if (typeof(response) == TYPE_STR && length(response) > 0)
          result_event = $event:mk_info(player, $format.block:mk(error_msg + "\n", response)):as_inset();
          player:rewrite_event(rewrite_id, result_event, current_conn);
        else
          player:rewrite_event(rewrite_id, error_msg, current_conn);
        endif
      except e (ANY)
        player:rewrite_event(rewrite_id, error_msg, current_conn);
      endtry
    endfork
    return true;
  endverb

  method _do_resolve_verb_name owner: HACKER
    "Resolve space-separated verb names to a single verb name.";
    caller == this || raise(E_PERM);
    {target_obj, verb_spec} = args;
    verb_spec = verb_spec:trim();
    if (!verb_spec)
      raise(E_INVARG, "Verb name cannot be blank.");
    endif
    if (!index(verb_spec, " "))
      return verb_spec;
    endif
    "Try each space-separated name and ensure they refer to one verb";
    names = verb_spec:words();
    found_info = 0;
    found_name = "";
    for name in (names)
      info = `this:_do_get_verb_info(target_obj, name) ! E_VERBNF => 0';
      if (!info)
        continue;
      endif
      if (!found_info)
        found_info = info;
        found_name = name;
      elseif (info[3] != found_info[3])
        raise(E_INVARG, "Verb name list refers to multiple verbs; use a single name.");
      endif
    endfor
    if (!found_info)
      raise(E_VERBNF, "Verb not found: " + verb_spec);
    endif
    return found_name;
  endmethod

  verb "@ps @tasks" (none none none) owner: ARCH_WIZARD flags: "rd"
    "USAGE: @ps -- Show active tasks, queued tasks, and live schedules visible to you.";
    this:_challenge_command_perms();
    set_task_perms(player);
    const active = active_tasks();
    const queued = queued_tasks();
    const now = time();
    let blocks = {};
    "Active tasks section";
    if (length(active) > 0)
      let active_rows = {};
      for task in (active)
        const {task_id, task_player, start_info} = task;
        let task_type = "";
        let task_detail = "";
        if (typeof(start_info) == TYPE_LIST && length(start_info) >= 1)
          task_type = tostr(start_info[1]);
          task_detail = length(start_info) >= 2 ? tostr(start_info[2]) | "";
        else
          task_type = tostr(start_info);
          task_detail = "";
        endif
        active_rows = {@active_rows, {tostr(task_id), tostr(task_player), task_type, task_detail}};
      endfor
      const active_table = $format.table:mk({"ID", "Player", "Type", "Start info"}, active_rows);
      blocks = {@blocks, $format.title:mk("Active Tasks", 3), active_table};
    else
      blocks = {@blocks, $format.title:mk("Active Tasks", 3), "(none)"};
    endif
    "Queued/suspended tasks section";
    if (length(queued) > 0)
      let queued_rows = {};
      for task in (queued)
        "Format: {task_id, start_time, 0, 0, programmer, verb_loc, verb_name, line, this}";
        const task_id = task[1];
        const resume_time = task[2];
        const programmer = task[5];
        const verb_loc = task[6];
        const verb_name = task[7];
        const line_num = task[8];
        "Calculate time until resume";
        const delta = resume_time - now;
        let time_str = "";
        if (delta > 0)
          if (delta < 60)
            time_str = "in " + tostr(delta) + "s";
          elseif (delta < 3600)
            time_str = "in " + tostr(delta / 60) + "m";
          else
            time_str = "in " + tostr(delta / 3600) + "h";
          endif
        elseif (delta == 0)
          time_str = "now";
        else
          time_str = "ready";
        endif
        let verb_str = tostr(verb_loc) + ":" + verb_name;
        if (line_num)
          verb_str = verb_str + " (line " + tostr(line_num) + ")";
        endif
        queued_rows = {@queued_rows, {tostr(task_id), tostr(programmer), time_str, verb_str}};
      endfor
      const queued_table = $format.table:mk({"ID", "Owner", "Resume", "Verb"}, queued_rows);
      blocks = {@blocks, $format.title:mk("Queued Tasks", 3), queued_table};
    else
      blocks = {@blocks, $format.title:mk("Queued Tasks", 3), "(none)"};
    endif
    const schedule_rows = $prog_features:_schedule_rows();
    blocks = {@blocks, $format.title:mk("Schedules", 3), length(schedule_rows) ? $prog_features:_schedule_table(schedule_rows) | "(none)"};
    const summary = tostr(length(active)) + " active, " + tostr(length(queued)) + " queued, " + tostr(length(schedule_rows)) + " scheduled";
    blocks = {@blocks, "", summary};
    const output = $format.block:mk(@blocks);
    player:inform_current($event:mk_info(player, output));
  endverb

  method _schedule_rows owner: ARCH_WIZARD
    "Return live schedule rows visible to caller_perms(); wizard callers see all owners.";
    set_task_perms(caller_perms());
    let rows = {};
    for id in (schedules())
      try
        const info = schedule_info(id);
        if (!info["retired"])
          const timing = info["kind"] == "at" ? "once" | (info["adaptive"] ? "adaptive, base " | "every ") + tostr(info["interval"]) + "s";
          rows = {@rows, {tostr(id), tostr(info["owner"]), tostr(info["target"]) + ":" + info["verb"], timing, $prog_features:_schedule_next(info), info["running_task"] ? tostr(info["running_task"]) | "-"}};
        endif
      except (E_INVARG)
        "A schedule can disappear between listing its ID and reading its diagnostics.";
      endtry
    endfor
    return rows;
  endmethod

  method _schedule_table owner: ARCH_WIZARD
    "Format schedule rows with schedule IDs distinct from running task IDs.";
    const {rows} = args;
    return $format.table:mk({"Schedule ID", "Owner", "Verb", "Timing", "Next", "Running task"}, rows);
  endmethod

  method _schedule_next owner: ARCH_WIZARD
    "Format a schedule deadline; adaptive schedules can wait for a firing to finish.";
    const {info} = args;
    if (info["next_run"] == 0.0)
      return info["running_task"] && !info["retired"] ? "after firing" | "-";
    endif
    const remaining = info["next_run"] - tofloat(time());
    return remaining > 0.0 ? "in " + tostr(max(1, toint(remaining))) + "s" | "ready";
  endmethod

  method _schedule_id owner: ARCH_WIZARD
    "Parse one positive decimal schedule ID; raise E_INVARG for malformed input.";
    const {text} = args;
    const trimmed = text:trim();
    match(trimmed, "^[0-9]+$") || raise(E_INVARG, "Expected a positive schedule ID.");
    const id = toint(trimmed);
    id > 0 || raise(E_INVARG, "Expected a positive schedule ID.");
    return id;
  endmethod

  verb "@schedules" (none none none) owner: ARCH_WIZARD flags: "rd"
    "USAGE: @schedules -- List your live schedules; wizards see all owners.";
    this:_challenge_command_perms();
    set_task_perms(player);
    const rows = $prog_features:_schedule_rows();
    const output = $format.block:mk($format.title:mk("Schedules", 3), length(rows) ? $prog_features:_schedule_table(rows) | "(none)", tostr(length(rows)) + " scheduled", "Use @schedule <id> for details or @stop-schedule <id> to stop future firings.");
    player:inform_current($event:mk_info(player, output));
  endverb

  verb "@schedule" (any none none) owner: ARCH_WIZARD flags: "rd"
    "USAGE: @schedule <id> -- Inspect a schedule you own; wizards may inspect any schedule.";
    this:_challenge_command_perms();
    set_task_perms(player);
    let id = 0;
    try
      id = $prog_features:_schedule_id(argstr);
    except (E_INVARG)
      player:inform_current($event:mk_error(player, "Usage: @schedule <positive schedule-id>"));
      return;
    endtry
    try
      const info = schedule_info(id);
      const rows = {{"Owner", tostr(info["owner"])}, {"Verb", tostr(info["target"]) + ":" + info["verb"]}, {"Arguments", toliteral(info["args"])}, {"Kind", info["kind"] == "at" ? "once" | "recurring"}, {"Base interval", tostr(info["interval"]) + "s"}, {"Adaptive", info["adaptive"] ? "yes" | "no"}, {"Next", $prog_features:_schedule_next(info)}, {"Running task", info["running_task"] ? tostr(info["running_task"]) | "-"}, {"Runs / faults", tostr(info["run_count"]) + " / " + tostr(info["fault_count"])}, {"Missed / overlap", tostr(info["missed_count"]) + " / " + tostr(info["overlap_count"])}, {"Catchup / overlap policy", info["catchup"] + " / " + info["overlap"]}, {"State", toliteral(info["state"])}, {"Last fault", toliteral(info["last_fault"])}, {"Status", info["retired"] ? "retired: " + info["retire_reason"] | "live"}};
      player:inform_current($event:mk_info(player, $format.block:mk($format.title:mk("Schedule " + tostr(id), 3), $format.table:mk({"Field", "Value"}, rows))));
    except (E_INVARG)
      player:inform_current($event:mk_error(player, "No such schedule: " + tostr(id)));
    except (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied: you don't own schedule " + tostr(id) + "."));
    endtry
  endverb

  verb "@stop-schedule @kill-schedule" (any none none) owner: ARCH_WIZARD flags: "rd"
    "USAGE: @stop-schedule <id> -- Stop future firings of your schedule; wizards may stop any schedule.";
    this:_challenge_command_perms();
    set_task_perms(player);
    let id = 0;
    try
      id = $prog_features:_schedule_id(argstr);
    except (E_INVARG)
      player:inform_current($event:mk_error(player, "Usage: @stop-schedule <positive schedule-id>"));
      return;
    endtry
    try
      if (schedule_stop(id))
        player:inform_current($event:mk_info(player, "Stopped schedule " + tostr(id) + ". Running firings are unchanged; use @kill <task-id> to stop one."));
      else
        player:inform_current($event:mk_info(player, "No live schedule: " + tostr(id)));
      endif
    except (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied: you don't own schedule " + tostr(id) + "."));
    endtry
  endverb

  verb "@kill-task @kill" (any any any) owner: ARCH_WIZARD flags: "rd"
    "USAGE: @kill <task-id> -- Kill a task by ID.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, "Usage: @kill <task-id>"));
      return;
    endif
    task_id = tonum(argstr:trim());
    if (!task_id)
      player:inform_current($event:mk_error(player, "Invalid task ID: " + argstr));
      return;
    endif
    try
      kill_task(task_id);
      player:inform_current($event:mk_info(player, "Killed task " + tostr(task_id) + "."));
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "No such task: " + tostr(task_id)));
    except e (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied: you don't own task " + tostr(task_id) + "."));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error killing task: " + tostr(e[2])));
    endtry
  endverb

  verb "@chparent" (any at any) owner: ARCH_WIZARD flags: "rd"
    "Change an object's parent.";
    "Usage: @chparent <object> to <new-parent> [--dry-run]";
    caller != player && raise(E_PERM);
    player.programmer || raise(E_PERM, "Programmer features required.");
    set_task_perms(player);
    if (!dobjstr || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@chparent <object> to <new-parent> [--dry-run]")));
      return;
    endif
    dry_run = argstr && index(argstr, "--dry-run") != 0;
    new_parent_spec = iobjstr:trim();
    if (dry_run)
      new_parent_spec = new_parent_spec:replace_all(" --dry-run", "");
      new_parent_spec = new_parent_spec:replace_all("--dry-run", "");
      new_parent_spec = new_parent_spec:trim();
    endif
    try
      target = $match:match_object(dobjstr, player);
      typeof(target) != TYPE_OBJ && raise(E_INVARG, "Target is not an object.");
      !valid(target) && raise(E_INVARG, "Target object no longer exists.");
      new_parent = $match:match_object(new_parent_spec, player);
      typeof(new_parent) != TYPE_OBJ && raise(E_INVARG, "New parent is not an object.");
      !valid(new_parent) && raise(E_INVARG, "New parent object no longer exists.");
      old_parent = parent(target);
      old_name = valid(old_parent) ? tostr(old_parent.name, " (", old_parent, ")") | "(none)";
      new_name = tostr(new_parent.name, " (", new_parent, ")");
      if (dry_run)
        player:inform_current($event:mk_info(player, tostr("Dry-run: would change parent of ", target.name, " (", target, ") from ", old_name, " to ", new_name)));
        return target;
      endif
      chparent(target, new_parent);
      message = tostr("Changed parent of ", target.name, " (", target, ") from ", old_name, " to ", new_name);
      player:inform_current($event:mk_info(player, message));
      return target;
    except e (ANY)
      message = length(e) >= 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      player:inform_current($event:mk_error(player, message));
      return false;
    endtry
  endverb

  method _find_verb_by_argspec owner: HACKER
    "Find a verb matching name and argspec, return its 1-based index or 0.";
    caller == this || raise(E_PERM);
    {target_obj, verb_name, req_dobj, req_prep, req_iobj} = args;
    verb_list = this:_do_get_verbs(target_obj);
    for i in [1..length(verb_list)]
      try
        info = this:_do_get_verb_info(target_obj, verb_list[i]);
        names = info[3]:split(" ");
        matched = false;
        for n in (names)
          if (n == verb_name)
            matched = true;
            break;
          endif
        endfor
        if (!matched)
          continue;
        endif
        {dobj, prep, iobj} = this:_do_get_verb_args(target_obj, verb_list[i]);
        if (dobj == req_dobj && prep == req_prep && iobj == req_iobj)
          return i;
        endif
      except (E_VERBNF)
        continue;
      except e (ANY)
        raise(e[1], "Error inspecting verb #" + tostr(i) + " on " + tostr(target_obj) + ": " + tostr(e[2]));
      endtry
    endfor
    return 0;
  endmethod

  verb "@program @program#" (any any any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>:<verb> [<dobj> <prep> <iobj>] -- Program a verb via line input.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@program OBJECT:VERB [DOBJ PREP IOBJ]\n@program# OBJECT:VERB-NUMBER\n@program OBJECT:VERB = CODE_LINE")));
      return;
    endif
    input = argstr:trim();
    "Inline one-line mode: @program OBJ:VERB = CODE";
    eq_pos = index(input, " = ");
    inline_code = "";
    if (eq_pos)
      inline_code = input[eq_pos + 3..$]:trim();
      input = input[1..eq_pos - 1]:trim();
      if (!inline_code)
        player:inform_current($event:mk_error(player, "Inline mode requires code after '='."));
        return;
      endif
    endif
    "Parse the verb reference";
    words = input:words();
    if (!words)
      player:inform_current($event:mk_error(player, "Invalid format. Use 'object:verb' or 'object:number'."));
      return;
    endif
    parsed = words[1]:parse_verbref();
    if (!parsed)
      player:inform_current($event:mk_error(player, "Invalid format. Use 'object:verb' or 'object:number'."));
      return;
    endif
    {object_str, verb_spec} = parsed;
    "Match the object";
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    "Determine verb descriptor based on which command was used";
    if (verb == "@program#")
      try
        verb_num = toint(verb_spec);
      except e (E_INVARG)
        player:inform_current($event:mk_error(player, "Invalid verb number: " + verb_spec));
        return;
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error parsing verb number: " + tostr(e[2])));
        return;
      endtry
      if (verb_num < 1)
        player:inform_current($event:mk_error(player, "Invalid verb number: " + verb_spec));
        return;
      endif
      verb_desc = verb_num;
      try
        info = verb_info(target_obj, verb_num);
        display_name = info[3]:split(" ")[1];
      except (E_VERBNF)
        player:inform_current($event:mk_error(player, "No verb #" + tostr(verb_num) + " on " + target_obj.name + "."));
        return;
      except (E_PERM)
        player:inform_current($event:mk_error(player, "Permission denied."));
        return;
      endtry
    elseif (length(words) >= 4)
      req_dobj = words[2];
      req_prep = words[3];
      req_iobj = words[4];
      if (!(req_dobj in {"none", "this", "any"}))
        player:inform_current($event:mk_error(player, "Direct object must be 'none', 'this', or 'any'"));
        return;
      endif
      if (!(req_iobj in {"none", "this", "any"}))
        player:inform_current($event:mk_error(player, "Indirect object must be 'none', 'this', or 'any'"));
        return;
      endif
      if (!$prog_utils:is_valid_prep(req_prep))
        player:inform_current($event:mk_error(player, "Invalid preposition: '" + req_prep + "'. Use 'none', 'any', or a valid preposition."));
        return;
      endif
      verb_desc = this:_find_verb_by_argspec(target_obj, verb_spec, req_dobj, req_prep, req_iobj);
      if (!verb_desc)
        player:inform_current($event:mk_error(player, "No verb '" + verb_spec + "' with args " + req_dobj + " " + req_prep + " " + req_iobj + " on " + target_obj.name + "."));
        return;
      endif
      display_name = verb_spec;
    else
      try
        verb_spec = this:_do_resolve_verb_name(target_obj, verb_spec);
      except (E_VERBNF)
        player:inform_current($event:mk_error(player, "Verb '" + verb_spec + "' not found on " + target_obj.name + "."));
        return;
      endtry
      try
        verb_info(target_obj, verb_spec);
      except (E_VERBNF)
        player:inform_current($event:mk_error(player, "Verb '" + verb_spec + "' not found on " + target_obj.name + "."));
        return;
      endtry
      verb_desc = verb_spec;
      display_name = verb_spec;
    endif
    if (inline_code)
      try
        errors = set_verb_code(target_obj, verb_desc, {inline_code}, 2, 1);
        if (!errors)
          player:inform_current($event:mk_info(player, "1 line programmed."));
        else
          player:inform_current($event:mk_error(player, $format.code:mk(errors:join("\n"))));
        endif
      except e (E_PERM)
        player:inform_current($event:mk_error(player, "Permission denied."));
      except e (ANY)
        player:inform_current($event:mk_error(player, tostr(e[1]) + ": " + tostr(e[2])));
      endtry
      return;
    endif
    "Enter line-reading mode";
    player:inform_current($event:mk_info(player, "Now programming " + tostr(target_obj) + ":" + display_name + ".  Use \".\" to finish, \"@abort\" to cancel."));
    lines = {};
    try
      while (1)
        line = read();
        if (line == ".")
          break;
        elseif (line == "@abort")
          player:inform_current($event:mk_info(player, "Programming aborted."));
          return;
        elseif (length(line) >= 1 && line[1] == ".")
          line = line[2..$];
        endif
        lines = {@lines, line};
      endwhile
    except e (ANY)
      player:inform_current($event:mk_error(player, "Read error: " + tostr(e[2])));
      return;
    endtry
    "Compile and install";
    try
      errors = set_verb_code(target_obj, verb_desc, lines, 2, 1);
      if (!errors)
        count = length(lines);
        player:inform_current($event:mk_info(player, tostr(count) + " line" + (count != 1 ? "s" | "") + " programmed."));
      else
        error_text = errors:join("\n");
        player:inform_current($event:mk_error(player, $format.code:mk(error_text)));
      endif
    except e (E_PERM)
      player:inform_current($event:mk_error(player, "Permission denied."));
    except e (ANY)
      player:inform_current($event:mk_error(player, tostr(e[1]) + ": " + tostr(e[2])));
    endtry
  endverb

  verb "@clear-p*roperty" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>.<property> -- Clear an overridden inherited property to inherit parent value.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@clear-property OBJECT.PROPERTY")));
      return;
    endif
    spec = argstr:trim();
    parsed = $prog_utils:parse_target_spec(spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Usage: @clear-property OBJECT.PROPERTY"));
      return;
    endif
    selectors = parsed['selectors];
    if (length(selectors) != 1)
      player:inform_current($event:mk_error(player, "Usage: @clear-property OBJECT.PROPERTY"));
      return;
    endif
    selector = selectors[1];
    if (selector['kind] != 'property || !selector['item_name] || selector['inherited])
      player:inform_current($event:mk_error(player, "Use a direct property reference: object.property"));
      return;
    endif
    object_str = parsed['object_str];
    prop_name = selector['item_name];
    try
      target_obj = $match:match_object(object_str, player);
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "I don't see '" + object_str + "' here."));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error matching object: " + tostr(e[2])));
      return;
    endtry
    try
      clear_property(target_obj, prop_name);
      player:inform_current($event:mk_info(player, "Property " + tostr(target_obj) + "." + prop_name + " now inherits from its parent value."));
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, "Property " + tostr(target_obj) + "." + prop_name + " cannot be cleared this way."));
    except e (E_PROPNF)
      player:inform_current($event:mk_error(player, "Property " + prop_name + " not found on " + tostr(target_obj) + "."));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error clearing property: " + tostr(e[2])));
    endtry
  endverb

  verb "@which @where-defined" (any none none) owner: ARCH_WIZARD flags: "rd"
    "HINT: <object>:<verb> -- Show where a verb is defined and its metadata.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@which OBJECT:VERB")));
      return;
    endif
    spec = argstr:trim();
    parsed = $prog_utils:parse_target_spec(spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Usage: @which OBJECT:VERB"));
      return;
    endif
    selectors = parsed['selectors];
    if (length(selectors) != 1)
      player:inform_current($event:mk_error(player, "Usage: @which OBJECT:VERB"));
      return;
    endif
    selector = selectors[1];
    if (selector['kind] != 'verb || !selector['item_name])
      player:inform_current($event:mk_error(player, "Usage: @which OBJECT:VERB"));
      return;
    endif
    object_str = parsed['object_str];
    verb_name = selector['item_name];
    try
      target_obj = this:_resolve_object_ref(object_str, player, "object");
    except e (E_INVARG)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find object: " + tostr(e[2])));
      return;
    endtry
    try
      resolved = this:_do_resolve_verb_name(target_obj, verb_name);
      definer = this:_do_find_verb_definer(target_obj, resolved);
      if (definer == #-1)
        player:inform_current($event:mk_error(player, "Verb '" + resolved + "' not found on " + tostr(target_obj) + " or ancestors."));
        return;
      endif
      listing = this:_do_get_verb_listing(definer, resolved, 0);
      {verb_owner, verb_flags, dobj, prep, iobj, code_lines} = listing;
      headers = {"Target", "Resolved", "Defined on", "Owner", "Flags", "Args", "Lines"};
      argspec = dobj + " " + prep + " " + iobj;
      row = {tostr(target_obj), resolved, tostr(definer), tostr(verb_owner), verb_flags, argspec, tostr(length(code_lines))};
      table = $format.table:mk(headers, {row});
      player:inform_current($event:mk_info(player, table));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error resolving verb: " + tostr(e[2])));
    endtry
  endverb

  verb "@mvverb" (any at any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <src-obj>:<verb> to <dest-obj>[:<new-verb>] [--dry-run|--confirm] -- Move a verb.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!argstr || !dobjstr || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@mvverb SRC_OBJ:VERB to DEST_OBJ[:NEW_VERB] --confirm\n@mvverb SRC_OBJ:VERB to DEST_OBJ[:NEW_VERB] --dry-run")));
      return;
    endif
    dry_run = index(argstr, "--dry-run") != 0;
    confirm = index(argstr, "--confirm") != 0;
    if (!dry_run && !confirm)
      player:inform_current($event:mk_error(player, "Refusing to move without confirmation. Add --confirm (or use --dry-run)."));
      return;
    endif
    src_spec = dobjstr:trim();
    dst_spec = iobjstr:trim();
    dst_spec = dst_spec:replace_all(" --dry-run", "");
    dst_spec = dst_spec:replace_all("--dry-run", "");
    dst_spec = dst_spec:replace_all(" --confirm", "");
    dst_spec = dst_spec:replace_all("--confirm", "");
    dst_spec = dst_spec:trim();
    parsed = $prog_utils:parse_target_spec(src_spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Source must be OBJECT:VERB"));
      return;
    endif
    selectors = parsed['selectors];
    if (length(selectors) != 1 || selectors[1]['kind] != 'verb || !selectors[1]['item_name])
      player:inform_current($event:mk_error(player, "Source must be OBJECT:VERB"));
      return;
    endif
    src_obj_str = parsed['object_str];
    src_name = selectors[1]['item_name];
    try
      src_obj = $match:match_object(src_obj_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find source object: " + tostr(e[2])));
      return;
    endtry
    if (":" in dst_spec)
      parsed_dst = dst_spec:parse_verbref();
      if (!parsed_dst)
        player:inform_current($event:mk_error(player, "Destination must be OBJECT or OBJECT:NEW_VERB"));
        return;
      endif
      {dst_obj_str, dst_name} = parsed_dst;
    else
      dst_obj_str = dst_spec;
      dst_name = src_name;
    endif
    try
      src_name = this:_do_resolve_verb_name(src_obj, src_name);
      dst_obj = $match:match_object(dst_obj_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Resolution failed: " + tostr(e[2])));
      return;
    endtry
    if (!dst_name)
      dst_name = src_name;
    endif
    if (dry_run)
      player:inform_current($event:mk_info(player, "Dry-run: would move " + tostr(src_obj) + ":" + src_name + " -> " + tostr(dst_obj) + ":" + dst_name));
      return;
    endif
    existing = `verb_info(dst_obj, dst_name) ! E_VERBNF => 0';
    if (existing)
      player:inform_current($event:mk_error(player, "Destination already has verb: " + tostr(dst_obj) + ":" + dst_name));
      return;
    endif
    src_definer = this:_do_find_verb_definer(src_obj, src_name);
    if (src_definer == #-1)
      player:inform_current($event:mk_error(player, "Source verb not found: " + src_name));
      return;
    endif
    listing = this:_do_get_verb_listing(src_definer, src_name, 1);
    {src_owner, src_flags, src_dobj, src_prep, src_iobj, src_code} = listing;
    new_owner = player.wizard ? src_owner | player;
    try
      this:_do_add_verb(dst_obj, {new_owner, src_flags, dst_name}, {src_dobj, src_prep, src_iobj});
      compile_errors = set_verb_code(dst_obj, dst_name, src_code, 2, 1);
      if (compile_errors)
        this:_do_delete_verb(dst_obj, dst_name);
        player:inform_current($event:mk_error(player, "Move copy step failed:\n" + compile_errors:join("\n")));
        return;
      endif
      this:_do_delete_verb(src_definer, src_name);
      player:inform_current($event:mk_info(player, "Moved " + tostr(src_obj) + ":" + src_name + " -> " + tostr(dst_obj) + ":" + dst_name));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Move failed: " + tostr(e[2])));
    endtry
  endverb

  verb "@cpverb" (any at any) owner: ARCH_WIZARD flags: "rd"
    "HINT: <src-obj>:<verb> to <dest-obj>[:<new-verb>] -- Copy a verb.";
    this:_challenge_command_perms();
    set_task_perms(player);
    if (!dobjstr || !iobjstr)
      player:inform_current($event:mk_error(player, $format.code:mk("@cpverb SRC_OBJ:VERB to DEST_OBJ[:NEW_VERB]")));
      return;
    endif
    src_spec = dobjstr:trim();
    dst_spec = iobjstr:trim();
    parsed = $prog_utils:parse_target_spec(src_spec);
    if (!parsed || parsed['type] != 'compound)
      player:inform_current($event:mk_error(player, "Source must be OBJECT:VERB"));
      return;
    endif
    selectors = parsed['selectors];
    if (length(selectors) != 1 || selectors[1]['kind] != 'verb || !selectors[1]['item_name])
      player:inform_current($event:mk_error(player, "Source must be OBJECT:VERB"));
      return;
    endif
    src_obj_str = parsed['object_str];
    src_name = selectors[1]['item_name];
    try
      src_obj = $match:match_object(src_obj_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find source object: " + tostr(e[2])));
      return;
    endtry
    try
      src_name = this:_do_resolve_verb_name(src_obj, src_name);
      src_definer = this:_do_find_verb_definer(src_obj, src_name);
      if (src_definer == #-1)
        player:inform_current($event:mk_error(player, "Source verb not found: " + src_name));
        return;
      endif
      listing = this:_do_get_verb_listing(src_definer, src_name, 1);
      {src_owner, src_flags, src_dobj, src_prep, src_iobj, src_code} = listing;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Error reading source verb: " + tostr(e[2])));
      return;
    endtry
    if (":" in dst_spec)
      parsed_dst = dst_spec:parse_verbref();
      if (!parsed_dst)
        player:inform_current($event:mk_error(player, "Destination must be OBJECT or OBJECT:NEW_VERB"));
        return;
      endif
      {dst_obj_str, dst_name} = parsed_dst;
    else
      dst_obj_str = dst_spec;
      dst_name = src_name;
    endif
    try
      dst_obj = $match:match_object(dst_obj_str, player);
    except e (ANY)
      player:inform_current($event:mk_error(player, "Could not find destination object: " + tostr(e[2])));
      return;
    endtry
    if (!dst_name)
      dst_name = src_name;
    endif
    existing = `verb_info(dst_obj, dst_name) ! E_VERBNF => 0';
    if (existing)
      player:inform_current($event:mk_error(player, "Destination already has verb: " + tostr(dst_obj) + ":" + dst_name));
      return;
    endif
    new_owner = player.wizard ? src_owner | player;
    try
      this:_do_add_verb(dst_obj, {new_owner, src_flags, dst_name}, {src_dobj, src_prep, src_iobj});
      compile_errors = set_verb_code(dst_obj, dst_name, src_code, 2, 1);
      if (compile_errors)
        this:_do_delete_verb(dst_obj, dst_name);
        player:inform_current($event:mk_error(player, "Copy failed to compile:\n" + compile_errors:join("\n")));
        return;
      endif
      msg = "Copied " + tostr(src_obj) + ":" + src_name + " -> " + tostr(dst_obj) + ":" + dst_name;
      msg = msg + " (flags " + src_flags + ", args " + src_dobj + " " + src_prep + " " + src_iobj + ")";
      player:inform_current($event:mk_info(player, msg));
    except e (ANY)
      player:inform_current($event:mk_error(player, "Copy failed: " + tostr(e[2])));
    endtry
  endverb

  method _resolve_object_ref owner: HACKER
    "Internal helper: resolve object references with better ambiguity diagnostics.";
    caller == this || raise(E_PERM);
    {ref_string, ?context = player, ?label = "object"} = args;
    typeof(ref_string) == TYPE_STR || raise(E_TYPE, "Object reference must be a string.");
    ref = ref_string:trim();
    ref || raise(E_INVARG, "Empty " + label + " reference.");
    if (ref[1] in {"#", "$", "@"} || ref in {"me", "player", "here"})
      return $match:match_object(ref, context);
    endif
    if (!valid(context))
      context = player;
    endif
    context_info = this:_do_get_object_match_info(context);
    scope = {};
    scope = {@scope, @context_info['contents]};
    if (valid(context_info['location]))
      location_info = this:_do_get_object_match_info(context_info['location]);
      scope = {@scope, @location_info['contents]};
    endif
    if (!scope)
      return $match:match_object(ref, context);
    endif
    scope_entries = {};
    for o in (scope)
      candidate_info = this:_do_get_object_match_info(o);
      entry = {o, candidate_info['name]};
      aliases = candidate_info['aliases];
      if (typeof(aliases) == TYPE_LIST)
        entry = {@entry, @aliases};
      endif
      scope_entries = {@scope_entries, entry};
    endfor
    result = $match:resolve_in_scope(ref, scope_entries, ['fuzzy_threshold -> 0.5]);
    if (result == $failed_match)
      raise(E_INVARG, "No " + label + " found matching '" + ref + "'.");
    endif
    if (result == $ambiguous_match)
      candidates = this:_matching_candidates(ref, context);
      if (!candidates)
        raise(E_INVARG, "Ambiguous " + label + " reference '" + ref + "'.");
      endif
      formatted = {};
      max_show = 8;
      count = 0;
      for o in (candidates)
        count = count + 1;
        if (count > max_show)
          break;
        endif
        candidate_info = this:_do_get_object_match_info(o);
        formatted = {@formatted, tostr(candidate_info['name], " (", o, ")")};
      endfor
      suffix = length(candidates) > max_show ? " ..." | "";
      msg = "Ambiguous " + label + " '" + ref + "'. Candidates: " + formatted:join(", ") + suffix;
      raise(E_INVARG, msg);
    endif
    return result;
  endmethod

  method _matching_candidates owner: HACKER
    "Internal helper: list likely candidate objects for a plain-name token in context.";
    caller == this || raise(E_PERM);
    {token, ?context = player} = args;
    typeof(token) == TYPE_STR || return {};
    valid(context) || return {};
    needle = token:trim():lowercase();
    !needle && return {};
    context_info = this:_do_get_object_match_info(context);
    scope = {};
    scope = {@scope, @context_info['contents]};
    if (valid(context_info['location]))
      location_info = this:_do_get_object_match_info(context_info['location]);
      scope = {@scope, @location_info['contents]};
    endif
    candidates = {};
    seen = [];
    for o in (scope)
      if (!(o in mapkeys(seen)))
        seen[o] = 1;
      else
        continue;
      endif
      candidate_info = this:_do_get_object_match_info(o);
      names = {candidate_info['name]};
      aliases = candidate_info['aliases];
      if (typeof(aliases) == TYPE_LIST)
        names = {@names, @aliases};
      endif
      matched = 0;
      for n in (names)
        if (typeof(n) == TYPE_STR)
          lower = n:lowercase();
          if (index(lower, needle) || index(needle, lower))
            matched = 1;
            break;
          endif
        endif
      endfor
      if (matched)
        candidates = {@candidates, o};
      endif
    endfor
    return candidates;
  endmethod

  method test_grep_object_literal_owner_filter owner: ARCH_WIZARD
    "Unit test: @grep helper finds literal matches and applies owner filtering.";
    fixture = #-1;
    try
      fixture = create($root);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_verb(fixture, {$hacker, "xd", "grep_hacker_match"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "grep_hacker_match", {"return \"phase_two_grep_token from hacker\";"}, 2, 1);
      $test_utils:assert_false(errors, "hacker-owned grep fixture verb should compile");
      add_verb(fixture, {$arch_wizard, "xd", "grep_wizard_match"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "grep_wizard_match", {"return \"phase_two_grep_token from wizard\";"}, 2, 1);
      $test_utils:assert_false(errors, "wizard-owned grep fixture verb should compile");
      result = this:_do_grep_object("phase_two_grep_token", fixture, false, false, $hacker);
      matches = result['matches];
      $test_utils:assert_eq(length(matches), 1, "owner filter should keep only hacker-owned match");
      $test_utils:assert_eq(matches[1][2], "grep_hacker_match", "literal grep should return the hacker-owned verb");
      $test_utils:assert_eq(result['skipped_verb_count], 0, "private fixture verbs should be read through scoped grants");
    finally
      if (valid(fixture))
        `delete_verb(fixture, "grep_hacker_match") ! E_VERBNF => 0';
        `delete_verb(fixture, "grep_wizard_match") ! E_VERBNF => 0';
        recycle(fixture);
      endif
    endtry
    return true;
  endmethod

  method test_grep_object_regex owner: ARCH_WIZARD
    "Unit test: @grep helper finds regex matches through the same result path.";
    fixture = #-1;
    try
      fixture = create($root);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_verb(fixture, {$hacker, "xd", "grep_regex_match"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "grep_regex_match", {"return \"phase_two_grep_token regex\";"}, 2, 1);
      $test_utils:assert_false(errors, "regex grep fixture verb should compile");
      result = this:_do_grep_object("phase_[a-z]+_grep_token", fixture, false, true, 0);
      matches = result['matches];
      $test_utils:assert_eq(length(matches), 1, "regex grep should find the matching verb");
      $test_utils:assert_eq(matches[1][2], "grep_regex_match", "regex grep should identify the matching verb");
      $test_utils:assert_eq(result['skipped_verb_count], 0, "private regex fixture verb should be read through scoped grants");
    finally
      if (valid(fixture))
        `delete_verb(fixture, "grep_regex_match") ! E_VERBNF => 0';
        recycle(fixture);
      endif
    endtry
    return true;
  endmethod

  method test_find_verb_by_argspec_returns_matching_index owner: ARCH_WIZARD
    "Unit test: argspec disambiguation finds the matching verb index.";
    fixture = #-1;
    try
      fixture = create($root);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_verb(fixture, {$hacker, "xd", "argspec_probe"}, {"this", "none", "this"});
      index = this:_find_verb_by_argspec(fixture, "argspec_probe", "this", "none", "this");
      $test_utils:assert_true(index > 0, "argspec helper should find the matching private verb index through scoped grants");
      missing = this:_find_verb_by_argspec(fixture, "argspec_probe", "none", "none", "none");
      $test_utils:assert_eq(missing, 0, "argspec helper should return 0 for non-matching args");
    finally
      if (valid(fixture))
        `delete_verb(fixture, "argspec_probe") ! E_VERBNF => 0';
        recycle(fixture);
      endif
    endtry
    return true;
  endmethod

  method test_verb_command_helpers_scoped_grants owner: ARCH_WIZARD
    "Unit test: verb command helpers use scoped grants for private object/verb operations.";
    fixture = #-1;
    try
      fixture = create($root);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_verb(fixture, {$hacker, "", "private_helper_probe"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "private_helper_probe", {"return 42;"}, 2, 1);
      $test_utils:assert_false(errors, "private helper probe should compile");
      $test_utils:assert_true(fixture.owner != player, "fixture should not be owned by the player");
      verb_list = this:_do_get_verbs(fixture);
      $test_utils:assert_true("private_helper_probe" in verb_list, "_do_get_verbs should use an object_read grant");
      listing = this:_do_get_verb_listing(fixture, "private_helper_probe", false);
      $test_utils:assert_eq(listing[6], {"return 42;"}, "_do_get_verb_listing should use a verb_read grant");
      argspec = this:_do_get_verb_args(fixture, "private_helper_probe");
      $test_utils:assert_eq(argspec, {"this", "none", "this"}, "_do_get_verb_args should use a verb_read grant");
      add_verb(fixture, {$hacker, "", "private resolve alias"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "private", {"return 99;"}, 2, 1);
      $test_utils:assert_false(errors, "private resolve alias should compile");
      resolved_name = this:_do_resolve_verb_name(fixture, "private resolve");
      $test_utils:assert_eq(resolved_name, "private", "_do_resolve_verb_name should use verb_read grants for hidden aliases");
      this:_do_check_verb_exists(fixture, "private_helper_probe");
      this:_do_set_verb_args(fixture, "private_helper_probe", {"none", "none", "none"});
      metadata = $prog_utils:get_verb_metadata(fixture, "private_helper_probe");
      $test_utils:assert_eq(metadata:dobj(), "none", "_do_set_verb_args should use a verb_write grant");
      this:_do_add_verb(fixture, {player, "rxd", "grant_added_probe"}, {"this", "none", "this"});
      $test_utils:assert_true("grant_added_probe" in verbs(fixture), "_do_add_verb should use a verb_add grant");
      this:_do_delete_verb(fixture, "grant_added_probe");
      $test_utils:assert_false("grant_added_probe" in verbs(fixture), "_do_delete_verb should use an object_write grant");
    finally
      if (valid(fixture))
        `delete_verb(fixture, "grant_added_probe") ! E_VERBNF => 0';
        `delete_verb(fixture, "private") ! E_VERBNF => 0';
        `delete_verb(fixture, "private_helper_probe") ! E_VERBNF => 0';
        recycle(fixture);
      endif
    endtry
    return true;
  endmethod

  method test_property_command_helpers_scoped_grants owner: ARCH_WIZARD
    "Unit test: property command helpers use scoped grants for private object/property operations.";
    fixture = #-1;
    try
      fixture = create($root);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_property(fixture, "private_helper_prop", "secret", {$hacker, ""});
      $test_utils:assert_true(fixture.owner != player, "fixture should not be owned by the player");
      prop_list = this:_do_get_properties(fixture);
      prop_names = {};
      for p in (prop_list)
        prop_names = {@prop_names, tostr(p)};
      endfor
      $test_utils:assert_true("private_helper_prop" in prop_names, "_do_get_properties should use an object_read grant");
      metadata = this:_do_get_property_metadata(fixture, "private_helper_prop");
      $test_utils:assert_eq(metadata.owner, $hacker, "_do_get_property_metadata should use a property_read grant");
      value = this:_do_get_property_value(fixture, "private_helper_prop");
      $test_utils:assert_eq(value, "secret", "_do_get_property_value should use a property_read grant");
      this:_do_add_property(fixture, "grant_added_prop", 17, {player, ""});
      prop_names = {};
      for p in (properties(fixture))
        prop_names = {@prop_names, tostr(p)};
      endfor
      $test_utils:assert_true("grant_added_prop" in prop_names, "_do_add_property should use a property_define grant");
      this:_do_delete_property(fixture, "grant_added_prop");
      prop_names = {};
      for p in (properties(fixture))
        prop_names = {@prop_names, tostr(p)};
      endfor
      $test_utils:assert_false("grant_added_prop" in prop_names, "_do_delete_property should use a property_delete grant");
    finally
      if (valid(fixture))
        `delete_property(fixture, "grant_added_prop") ! E_PROPNF => 0';
        `delete_property(fixture, "private_helper_prop") ! E_PROPNF => 0';
        recycle(fixture);
      endif
    endtry
    return true;
  endmethod

  method test_show_display_helpers_scoped_grants owner: ARCH_WIZARD
    "Unit test: @show display helpers use scoped grants for private object/property/verb reads.";
    parent_fixture = #-1;
    fixture = #-1;
    stubbed_inform = false;
    added_inform_log = false;
    try
      add_property(player, "prog_show_last_inform", {}, {$arch_wizard, "r"});
      added_inform_log = true;
      add_verb(player, {$arch_wizard, "rxd", "inform_current"}, {"this", "none", "this"});
      errors = set_verb_code(player, "inform_current", {"this.prog_show_last_inform = {@this.prog_show_last_inform, args[1]};", "return true;"}, 2, 1);
      $test_utils:assert_false(errors, "temporary inform_current stub should compile");
      stubbed_inform = true;
      parent_fixture = create($root);
      parent_fixture.owner = $hacker;
      parent_fixture.r = 0;
      parent_fixture.w = 0;
      add_property(parent_fixture, "private_display_parent_prop", "parent secret", {$hacker, ""});
      add_verb(parent_fixture, {$hacker, "", "private_display_parent_verb"}, {"this", "none", "this"});
      errors = set_verb_code(parent_fixture, "private_display_parent_verb", {"return \"parent secret\";"}, 2, 1);
      $test_utils:assert_false(errors, "private parent display probe should compile");
      fixture = create(parent_fixture);
      fixture.owner = $hacker;
      fixture.r = 0;
      fixture.w = 0;
      add_property(fixture, "private_display_prop", "secret", {$hacker, ""});
      add_verb(fixture, {$hacker, "", "private_display_verb"}, {"this", "none", "this"});
      errors = set_verb_code(fixture, "private_display_verb", {"return \"secret\";"}, 2, 1);
      $test_utils:assert_false(errors, "private display probe should compile");
      this:_display_header(fixture);
      this:_display_summary(fixture);
      this:_display_property(fixture, "private_display_prop");
      this:_display_inherited_property(fixture, "private_display_parent_prop");
      this:_display_all_properties(fixture, true);
      this:_display_verb(fixture, "private_display_verb");
      this:_display_inherited_verb(fixture, "private_display_parent_verb");
      this:_display_all_verbs(fixture, true);
      events = player.prog_show_last_inform;
      $test_utils:assert_eq(length(events), 8, "each display helper should emit an event");
      for event in (events)
        $test_utils:assert_true(event:validate(), "display helper should emit a valid event");
      endfor
      const local_property = $document:encode(events[3]:resolve(player))["blocks"][1]["rows"][1]["cells"][1]["blocks"][1]["children"][1];
      const inherited_property = $document:encode(events[4]:resolve(player))["blocks"][1]["rows"][1]["cells"][1]["blocks"][1]["children"][1];
      const local_verb = $document:encode(events[6]:resolve(player))["blocks"][1]["rows"][1]["cells"][1]["blocks"][1]["children"][1];
      const inherited_verb = $document:encode(events[7]:resolve(player))["blocks"][1]["rows"][1]["cells"][1]["blocks"][1]["children"][1];
      $test_utils:assert_eq(local_property["children"][1]["text"], ".private_display_prop", "local property should be displayed");
      $test_utils:assert_eq(inherited_property["children"][1]["text"], ".private_display_parent_prop", "inherited property should be displayed");
      $test_utils:assert_eq(local_verb["children"][1]["text"], tostr(fixture) + ":private_display_verb", "local verb should be displayed");
      $test_utils:assert_eq(inherited_verb["children"][1]["text"], tostr(parent_fixture) + ":private_display_parent_verb", "inherited verb should be displayed");
      const receiver_ref = $url_utils:to_curie_str(fixture);
      const parent_ref = $url_utils:to_curie_str(parent_fixture);
      $test_utils:assert_eq(local_property["descriptor"], ["kind" -> "property", "object" -> receiver_ref, "name" -> "private_display_prop"], "local property reference should target the inspected object");
      $test_utils:assert_eq(inherited_property["descriptor"], ["kind" -> "property", "object" -> receiver_ref, "name" -> "private_display_parent_prop"], "inherited property reference should retain the inspected object's value");
      $test_utils:assert_eq(local_verb["descriptor"], ["kind" -> "verb", "receiver" -> receiver_ref, "name" -> "private_display_verb", "definer" -> receiver_ref], "local verb reference should identify its receiver and definer");
      $test_utils:assert_eq(inherited_verb["descriptor"], ["kind" -> "verb", "receiver" -> receiver_ref, "name" -> "private_display_parent_verb", "definer" -> parent_ref], "inherited verb reference should distinguish receiver from definer");
    finally
      stubbed_inform && `delete_verb(player, "inform_current") ! E_VERBNF => 0';
      added_inform_log && `delete_property(player, "prog_show_last_inform") ! E_PROPNF => 0';
      if (valid(fixture))
        `delete_verb(fixture, "private_display_verb") ! E_VERBNF => 0';
        `delete_property(fixture, "private_display_prop") ! E_PROPNF => 0';
        recycle(fixture);
      endif
      if (valid(parent_fixture))
        `delete_verb(parent_fixture, "private_display_parent_verb") ! E_VERBNF => 0';
        `delete_property(parent_fixture, "private_display_parent_prop") ! E_PROPNF => 0';
        recycle(parent_fixture);
      endif
    endtry
    return true;
  endmethod

  method test_object_resolver_helpers_scoped_grants owner: ARCH_WIZARD
    "Unit test: object resolver helpers use scoped grants for private context matching.";
    context = #-1;
    target = #-1;
    try
      context = create($root);
      context.owner = $hacker;
      context.r = 0;
      context.w = 0;
      target = create($root);
      target.owner = $hacker;
      target.r = 0;
      target.w = 0;
      target:set_name_aliases("private resolver target", {"hidden-resolver"});
      target:moveto(context);
      resolved = this:_resolve_object_ref("hidden-resolver", context, "object");
      $test_utils:assert_eq(resolved, target, "_resolve_object_ref should match private contents through object_read grants");
      candidates = this:_matching_candidates("resolver", context);
      $test_utils:assert_true(target in candidates, "_matching_candidates should inspect private candidate names through object_read grants");
    finally
      valid(target) && recycle(target);
      valid(context) && recycle(context);
    endtry
    return true;
  endmethod
endobject
