object OBJ_UTILS [
  import_export_id -> "obj_utils",
  import_export_hierarchy -> {"utils"}
]
  name: "Object Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Core object utilities for introspection and formatting. Provides common functionality for displaying object information, verb signatures, and other object-related utilities.";

  method command_entry owner: ARCH_WIZARD
    "Render an invokable command signature, with a separate source reference for programmers.";
    set_task_perms(caller_perms());
    const {receiver, spec, viewer, ?label = "", ?actions = {}} = args;
    const {names, definer, direct, prep_spec, indirect} = spec;
    const name = strsub(names:words()[1], "*", "");
    let caption = label || name;
    let invocation = 0;
    let hint = definer:command_action(names, direct == "this" || indirect == "this" ? receiver:name() | "");
    "Object-authored actions retain their argument scopes and chosen prepositions.";
    for action in (actions)
      const command = action["command"];
      if (!(command:words()[1] in strsub(names, "*", ""):words()))
        continue;
      endif
      const parsed = parse_command(strsub(command, "{input}", "#-2"), {}, false);
      if (direct == "this" && parsed['dobjstr] != tostr(receiver) || (indirect == "this" && parsed['iobjstr] != tostr(receiver)) || (direct == "none" && parsed['dobjstr]) || (indirect == "none" && parsed['iobjstr]) || (prep_spec == "none" && parsed['prepstr]) || (!(prep_spec in {"none", "any"}) && !(parsed['prepstr] in prep_spec:split("/"))))
        continue;
      endif
      maphaskey(action, "action") && (hint = action["action"]);
      if (!maphaskey(action, "input"))
        invocation = $format.annotation:command(command, caption);
        continue;
      endif
      const input = action["input"];
      const slot = parsed['dobjstr] == "#-2" ? "dobj" | "iobj";
      let field = ["label" -> input["label"], "expectedKind" -> "object"];
      maphaskey(input, "suggestions") && (field["suggestions"] = input["suggestions"]);
      invocation = $format.annotation:command_template(strsub(command, "{input}", "{" + slot + "}"), [slot -> field], caption);
    endfor
    if (typeof(invocation) != TYPE_FLYWEIGHT)
      let template = name;
      let fields = [];
      if (prep_spec == "any")
        "Argspec cannot choose a preposition or describe a free-form command's grammar.";
        template = template + " {dobj}";
        fields["dobj"] = ["label" -> "Arguments", "expectedKind" -> "text", "required" -> false];
      else
        const prep = prep_spec == "none" ? "" | this:_preferred_prep(prep_spec);
        for position in ({{"dobj", direct, "Object"}, {"iobj", indirect, "Object"}})
          const {slot, constraint, prompt} = position;
          slot == "iobj" && prep && (template = template + " " + prep);
          if (constraint == "this")
            template = template + " " + tostr(receiver);
          elseif (constraint == "any")
            template = template + " {" + slot + "}";
            fields[slot] = ["label" -> prompt, "expectedKind" -> "object", "required" -> direct == "this" || indirect == "this" || prep_spec != "none", "suggestions" -> ["provider" -> $url_utils:to_curie_str(viewer), "source" -> "nearby"]];
          endif
        endfor
      endif
      invocation = length(fields) ? $format.annotation:command_template(template, fields, caption) | $format.annotation:command(template, caption);
    endif
    let descriptor = invocation.descriptor;
    const templated = maphaskey(descriptor, 'template);
    caption = templated ? descriptor['template] | descriptor['command];
    if (templated)
      let fields = descriptor['arguments];
      const labels = maphaskey(hint, "arguments") ? hint["arguments"] | [];
      for slot in (mapkeys(fields))
        let field = fields[slot];
        const field_label = maphaskey(labels, tostr(slot)) && typeof(labels[tostr(slot)]) == TYPE_STR && labels[tostr(slot)] ? labels[tostr(slot)] | field['label];
        field['label] = field_label;
        fields[slot] = field;
        caption = strsub(caption, "{" + tostr(slot) + "}", "<" + field_label:lowercase() + ">");
      endfor
      descriptor['arguments] = fields;
    endif
    caption = label || strsub(caption, tostr(receiver), receiver:name());
    invocation = $document_template:mk('reference, $format:inlines({caption}), ['descriptor -> descriptor]);
    return this:command_with_source(invocation:with_action(hint), receiver, names, definer, viewer);
  endmethod

  method command_with_source owner: ARCH_WIZARD
    "Keep invocation primary and offer programmer navigation as a separate adjacent link.";
    set_task_perms(caller_perms());
    const {invocation, receiver, name, definer, viewer} = args;
    const linked = maphaskey(invocation.descriptor, 'action) ? invocation | invocation:with_action(definer:command_action(name));
    !viewer.programmer && return linked;
    return $format.paragraph:inline(linked, " ", $format.annotation:verb(receiver, name, "\u2197", definer));
  endmethod

  method format_verb_signature owner: ARCH_WIZARD
    "Format a verb signature into user-friendly text. Returns a formatted string.";
    "Args: {verb_name, dobj, prep, iobj, ?object_name}";
    "If object_name is provided, uses it for 'this' placeholders. Otherwise uses <something>.";
    set_task_perms(caller_perms());
    {verb_name, dobj, prep, iobj, ?object_name = "<something>"} = args;
    verb_sig = verb_name;
    "Add dobj: use object name for 'this', placeholder for 'any'";
    if (dobj == "any")
      verb_sig = verb_sig + " <anything>";
    elseif (dobj == "this")
      verb_sig = verb_sig + " " + object_name;
    endif
    "Add iobj if there's a preposition: use object name for 'this', placeholder for 'any'";
    if (prep != "none")
      verb_sig = verb_sig + " " + prep;
      if (iobj == "any")
        verb_sig = verb_sig + " <anything>";
      elseif (iobj == "this")
        verb_sig = verb_sig + " " + object_name;
      endif
    endif
    return verb_sig;
  endmethod

  method format_verb_signatures owner: ARCH_WIZARD
    "Format multiple verb signatures from examination data. Returns a list of formatted strings.";
    "Args: {verb_list, ?object_name}";
    "verb_list should be from examination().verbs, where each item is {name, definer, dobj, prep, iobj}";
    set_task_perms(caller_perms());
    {verb_list, ?object_name = "<thing>"} = args;
    typeof(verb_list) != TYPE_LIST && return {};
    verb_sigs = {};
    for verb_info in (verb_list)
      {verb_name, definer, dobj, prep, iobj} = verb_info;
      sig = this:format_verb_signature(verb_name, dobj, prep, iobj, object_name);
      verb_sigs = {@verb_sigs, sig};
    endfor
    return verb_sigs;
  endmethod

  method _preferred_prep owner: ARCH_WIZARD
    "Spell a preposition for help with its shortest form: \"on\", not \"on top of\"; \"from\", not \"out of\".";
    "Ties keep the first spelling. UI labels do not choose parser words.";
    caller == this || raise(E_PERM);
    const {prep_spec} = args;
    const spellings = prep_spec:split("/");
    let best = spellings[1];
    for spelling in (spellings)
      length(spelling) < length(best) && (best = spelling);
    endfor
    return best;
  endmethod

  method _listable_command owner: ARCH_WIZARD
    "Exclude hook overrides and symbol shorthands from command listings; they are not commands to type.";
    caller == this || raise(E_PERM);
    const {definer, verb_name} = args;
    const first = verb_name:words()[1];
    length(first) || return false;
    "Shorthands such as ` and | and http:// run when typed but read as noise in a list of commands.";
    (index("abcdefghijklmnopqrstuvwxyz@", first[1]) && !index(first, "://")) || return false;
    "A verb that overrides an ancestor's method of the same name is a hook, such as maybe_handle_command.";
    for ancestor in (ancestors(definer))
      if (verb_name in `verbs(ancestor) ! ANY => {}'
          && `verb_args(ancestor, verb_name) ! ANY => {}' == {"this", "none", "this"})
        return false;
      endif
    endfor
    return true;
  endmethod

  method test_listable_commands owner: ARCH_WIZARD
    "Hook overrides and symbol shorthands are not listed as commands; ordinary verbs are.";
    const room = create($room);
    try
      add_verb(room, {$arch_wizard, "rxd", "maybe_handle_command"}, {"none", "none", "none"});
      add_verb(room, {$arch_wizard, "rxd", "ring"}, {"none", "none", "none"});
      const listed = {entry["verb"] for entry in (this:collect_ambient_verbs({room}))};
      $test_utils:assert_true("ring" in listed, "an ordinary room verb is listed");
      $test_utils:assert_false("maybe_handle_command" in listed, "a hook override is not listed");
      const player_listed = {item["verb"] for item in (this:collect_ambient_verbs({$social_features}))};
      $test_utils:assert_false("|*" in player_listed || "http://* https://*" in player_listed, "symbol shorthands are not listed");
    finally
      recycle(room);
    endtry
    return true;
  endmethod

  method is_targetable_verb owner: ARCH_WIZARD
    "Check if a verb targets specific objects (has dobj or iobj as 'this').";
    "A targetable verb requires you to specify an object. Ambient verbs do not.";
    "Args: {dobj, prep, iobj}";
    set_task_perms(caller_perms());
    {dobj, prep, iobj} = args;
    "Targetable if either dobj or iobj is 'this'";
    return dobj == "this" || iobj == "this";
  endmethod

  method collect_targetable_verbs owner: ARCH_WIZARD
    "Best-effort scanner for targetable verbs used by help and command suggestions.";
    "Returns list of maps: {object_ref, object_name, verbs}";
    "Args: {objects_list}";
    "Do not use this for authoritative command, permission, or metadata decisions; use strict metadata helpers instead.";
    set_task_perms(caller_perms());
    {obj_list} = args;
    typeof(obj_list) != TYPE_LIST && return {};
    result = {};
    seen_objects = [];
    caller_obj = `caller ! E_INVIND => #-1';
    for o in (obj_list)
      if (!valid(o))
        continue;
      endif
      "Skip the player themselves";
      if (o == caller_obj)
        continue;
      endif
      "Skip if we've already processed this object";
      if (maphaskey(seen_objects, tostr(o)))
        continue;
      endif
      seen_objects[tostr(o)] = true;
      "Get examination data";
      exam = o:examination();
      if (typeof(exam) != TYPE_FLYWEIGHT || !exam.verbs)
        continue;
      endif
      "Filter for targetable verbs only";
      targetable_sigs = {};
      const actions = o:inspection_commands(player);
      "Fixed objects inherit $thing's get and drop, which only refuse; do not offer them.";
      const fixed = !`o.portable ! E_PROPNF => true';
      for verb_info in (exam.verbs)
        {verb_name, definer, dobj, prep, iobj} = verb_info;
        if (fixed && definer == $thing && verb_name in {"get", "drop"})
          continue;
        endif
        if (this:is_targetable_verb(dobj, prep, iobj))
          sig = this:command_entry(o, verb_info, player, "", actions);
          targetable_sigs = {@targetable_sigs, sig};
        endif
      endfor
      if (targetable_sigs && length(targetable_sigs) > 0)
        entry = ["object" -> tostr(o), "object_name" -> exam.name, "verbs" -> targetable_sigs];
        result = listappend(result, entry);
      endif
    endfor
    return result;
  endmethod

  method collect_ambient_verbs owner: ARCH_WIZARD
    "Best-effort scanner for ambient verbs used by help and command suggestions.";
    "Ambient verbs don't target specific objects. Returns list of maps: {verb, from_object, from_name}";
    "Args: {objects_list}";
    "Do not use this for authoritative command, permission, or metadata decisions; use strict metadata helpers instead.";
    set_task_perms(caller_perms());
    {obj_list} = args;
    typeof(obj_list) != TYPE_LIST && return {};
    result = {};
    seen = [];
    for o in (obj_list)
      if (!valid(o))
        continue;
      endif
      "Get object display name for tracking source";
      obj_name = `o:display_name() ! ANY => tostr(o)';
      const actions = o:inspection_commands(player);
      "Walk inheritance chain to collect all verbs";
      ancestor_chain = ancestors(o);
      for definer in ({o, @ancestor_chain})
        if (!valid(definer))
          continue;
        endif
        "Get verbs defined at this level";
        all_verbs = `verbs(definer) ! ANY => {}';
        for verb_name in (all_verbs)
          "Get verb signature";
          verb_sig = `verb_args(definer, verb_name) ! ANY => {}';
          if (typeof(verb_sig) != TYPE_LIST || length(verb_sig) < 3)
            continue;
          endif
          {dobj, prep, iobj} = verb_sig;
          "Include ambient verbs (those that don't require 'this' as dobj/iobj)";
          if (!this:is_targetable_verb(dobj, prep, iobj) && this:_listable_command(definer, verb_name))
            "Skip if we've already seen this verb";
            if (!maphaskey(seen, verb_name))
              seen[verb_name] = true;
              entry = ["verb" -> verb_name, "from_object" -> o, "from_name" -> obj_name, "annotation" -> this:command_entry(o, {verb_name, definer, dobj, prep, iobj}, player, strsub(verb_name:words()[1], "*", ""), actions)];
              result = listappend(result, entry);
            endif
          endif
        endfor
      endfor
    endfor
    return result;
  endmethod

  method validate_and_compile_template owner: ARCH_WIZARD
    "Compile a template string into a $sub flyweight list. Returns {success, result}.";
    "If success is true, result is the compiled list.";
    "If success is false, result is the error message.";
    set_task_perms(caller_perms());
    const {template_string, ?target = false, ?property = ""} = args;
    typeof(template_string) != TYPE_STR && raise(E_TYPE, "template_string must be string");
    try
      const bindings = typeof(target) == TYPE_OBJ ? target:message_bindings(property) | false;
      const compiled = $sub_utils:compile(template_string, bindings);
      return {true, compiled};
    except e (ANY)
      const error_msg = length(e) > 2 && typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e);
      return {false, error_msg};
    endtry
  endmethod

  method set_compiled_message owner: ARCH_WIZARD
    "Replace a template using native authority for the property that actually changes.";
    const {target_obj, prop_name, compiled_list, who} = args;
    const principal = caller_perms();
    valid(who) && (principal == who || principal.wizard) || raise(E_PERM);
    set_task_perms(who);
    const prop_key = $obj_utils:property_key(target_obj, tostr(prop_name));
    prop_key == E_PROPNF && raise(E_PROPNF, "Property not found: " + tostr(prop_name));
    const existing = target_obj.(prop_key);
    if (typeof(existing) == TYPE_OBJ && isa(existing, $msg_bag))
      existing.entries = {compiled_list};
    elseif (typeof(existing) == TYPE_FLYWEIGHT && existing.delegate == $msg_bag)
      target_obj.(prop_key) = $msg_bag:mk(compiled_list);
    else
      target_obj.(prop_key) = compiled_list;
    endif
  endmethod

  method add_message_entry owner: ARCH_WIZARD
    "Append an entry under the actor's property permissions, creating a value bag if needed.";
    const {target_obj, property, compiled_entry, who} = args;
    const principal = caller_perms();
    valid(who) && (principal == who || principal.wizard) || raise(E_PERM);
    set_task_perms(who);
    const prop_name = tostr(property);
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    if (prop_key == E_PROPNF)
      add_property(target_obj, prop_name, $msg_bag:mk(compiled_entry), {who, "rc"});
      return;
    endif
    const existing = target_obj.(prop_key);
    if ($msg_bag:is_msg_bag(existing))
      if (typeof(existing) == TYPE_OBJ)
        existing.entries = {@existing.entries, compiled_entry};
      else
        target_obj.(prop_key) = toflyweight($msg_bag, flyslots(existing), {@flycontents(existing), compiled_entry});
      endif
    else
      target_obj.(prop_key) = $msg_bag:mk(compiled_entry);
    endif
  endmethod

  method remove_message_entry owner: ARCH_WIZARD
    "Remove an entry using native permissions on its object bag or containing property.";
    const {target_obj, property, index, who} = args;
    const principal = caller_perms();
    valid(who) && (principal == who || principal.wizard) || raise(E_PERM);
    set_task_perms(who);
    const prop_name = tostr(property);
    const prop_key = $obj_utils:property_key(target_obj, prop_name);
    prop_key == E_PROPNF && raise(E_INVARG, "Message bag not found on " + tostr(target_obj) + "." + prop_name);
    const existing = target_obj.(prop_key);
    if ($msg_bag:is_msg_bag(existing))
      if (typeof(existing) == TYPE_OBJ)
        index >= 1 && index <= length(existing.entries) || raise(E_RANGE, "Index out of range");
        existing.entries = listdelete(existing.entries, index);
      else
        const entries = flycontents(existing);
        index >= 1 && index <= length(entries) || raise(E_RANGE, "Index out of range");
        target_obj.(prop_key) = toflyweight($msg_bag, flyslots(existing), listdelete(entries, index));
      endif
    elseif (typeof(existing) == TYPE_LIST)
      index >= 1 && index <= length(existing) || raise(E_RANGE, "Index out of range");
      target_obj.(prop_key) = listdelete(existing, index);
    else
      raise(E_INVARG, "Message bag not found on " + tostr(target_obj) + "." + prop_name);
    endif
  endmethod

  method property_key owner: ARCH_WIZARD
    "Return the real property key matching a string property name, or E_PROPNF.";
    const {target_obj, prop_name} = args;
    set_task_perms(caller_perms());
    typeof(target_obj) != TYPE_OBJ && raise(E_TYPE, "target_obj must be object");
    typeof(prop_name) != TYPE_STR && raise(E_TYPE, "prop_name must be string");
    for existing_prop in ($obj_utils:property_keys(target_obj))
      if (tostr(existing_prop) == prop_name)
        return existing_prop;
      endif
    endfor
    return E_PROPNF;
  endmethod

  method property_keys owner: ARCH_WIZARD
    "List native property names without invoking overridable object code.";
    const {target} = args;
    set_task_perms(caller_perms());
    let names = {};
    for definer in ({target, @ancestors(target)})
      names = {@`properties(definer) ! E_PERM => {}', @names};
    endfor
    return names;
  endmethod

  method message_properties owner: ARCH_WIZARD
    "Return readable message names and values under the authenticated actor.";
    const {target, ?actor = caller_perms()} = args;
    set_task_perms(caller_perms());
    return $obj_utils:_matching_properties(target, actor, {"_msg", "_msgs", "_msg_bag"});
  endmethod

  method rule_properties owner: ARCH_WIZARD
    "Return readable rule names and values under the authenticated actor.";
    const {target, ?actor = caller_perms()} = args;
    set_task_perms(caller_perms());
    return $obj_utils:_matching_properties(target, actor, {"_rule"});
  endmethod

  method reaction_properties owner: ARCH_WIZARD
    "Return readable reaction definitions under the authenticated actor.";
    const {target, ?actor = caller_perms()} = args;
    set_task_perms(caller_perms());
    return $obj_utils:_matching_properties(target, actor, {"_reaction"}, $reaction);
  endmethod

  method _matching_properties owner: ARCH_WIZARD
    "Share enumeration without granting access to unreadable properties or another actor.";
    const {target, actor, suffixes, ?delegate = false} = args;
    const principal = caller_perms();
    valid(actor) && (principal == actor || principal.wizard) || raise(E_PERM);
    set_task_perms(actor);
    valid(target) || return {};
    let result = {};
    for property in ($obj_utils:property_keys(target))
      const name = tostr(property);
      let matches = false;
      for suffix in (suffixes)
        if (name:ends_with(suffix))
          matches = true;
          break;
        endif
      endfor
      if (!matches)
        continue;
      endif
      let value;
      try
        value = target.(property);
      except (E_PERM)
        continue;
      endtry
      if (typeof(delegate) == TYPE_OBJ
          && !(typeof(value) == TYPE_FLYWEIGHT && value.delegate == delegate))
        continue;
      endif
      result = {@result, {property, value}};
    endfor
    return result;
  endmethod

  method capture_legacy_naming owner: ARCH_WIZARD
    "Capture old naming data before upgrading. Return records and review notes; never change objects.";
    caller_perms().wizard || raise(E_PERM);
    const {objects} = args;
    let records = {};
    let review = {};
    for target in (objects)
      valid(target) || raise(E_INVARG, "A conversion target no longer exists.");
      const old_fields = {"is_proper_noun_name", "is_countable_noun", "is_plural_noun"};
      let old = [];
      let local = [];
      for field in (old_fields)
        const value = `target.(field) ! E_PROPNF => E_PROPNF';
        if (value != E_PROPNF)
          old[field] = value;
          local[field] = !is_clear_property(target, field);
        endif
      endfor
      let changes = [];
      if ((`local[old_fields[1]] ! E_RANGE => false') || (`local[old_fields[2]] ! E_RANGE => false'))
        changes['kind] = (`old[old_fields[1]] ! E_RANGE => false') ? 'name
          | ((`old[old_fields[2]] ! E_RANGE => true') ? 'count | 'mass);
      endif
      if (`local[old_fields[3]] ! E_RANGE => false')
        changes['number] = old[old_fields[3]] ? 'plural | 'singular;
      endif
      const article = `target.indefinite_article ! E_PROPNF => E_PROPNF';
      if (article != E_PROPNF && !is_clear_property(target, "indefinite_article"))
        changes['article] = article;
      endif
      isa(target, $mailbox) && (changes['kind] = 'name);
      let custom = {};
      for name in (verbs(target))
        const aliases = verb_info(target, name)[3]:words();
        if (length({alias for alias in (aliases) if alias in {"is_plural", "is_countable", "is_proper_noun", "display_name"}})
          && !(target in {$root, $thing, $wearable, $sub}))
          custom = {@custom, name};
        endif
      endfor
      const first = target.name:first_word();
      if (custom || (first && first[1] in {"a", "an", "the", "some"}))
        review = {@review, ['object -> target, 'name -> target.name, 'verbs -> custom,
          'reason -> custom ? "Convert custom naming verbs before applying this record."
            | "Review the article in this name. No automatic rename will occur."]};
      endif
      const letter = target != $letter && isa(target, $letter);
      if (length(changes) || letter)
        records = {@records, ['object -> target, 'name -> target.name, 'parent -> parent(target),
          'owner -> target.owner, 'settings -> changes, 'custom -> custom, 'letter -> letter,
          'subject -> letter && target.name != "letter" ? target.name | ""]};
      endif
    endfor
    return ['records -> records, 'review -> review];
  endmethod

  method apply_legacy_naming owner: ARCH_WIZARD
    "Apply saved records after the new naming API is installed. Caller retains the conversion data.";
    caller_perms().wizard || raise(E_PERM);
    const {records} = args;
    "Validate the whole batch before writing; custom overrides require an explicitly reviewed record.";
    for record in (records)
      const target = record['object];
      valid(target) && target.owner == record['owner] && parent(target) == record['parent]
        || raise(E_INVARG, "Conversion target changed owner, parent, or validity.");
      !record['custom] || raise(E_INVARG, "Convert custom naming verbs and clear this record's custom list after review.");
      strcmp(target.name, record['name]) == 0 || (record['letter] && target.name == "letter" && target.subject == record['subject])
        || raise(E_INVARG, "Conversion target was renamed: " + tostr(target));
      if (record['letter])
        typeof(target.subject) == TYPE_STR || raise(E_TYPE, "Install the letter subject property before conversion.");
      endif
      const settings = record['settings];
      for field in (mapkeys(settings))
        field in {'kind, 'number, 'article} || raise(E_INVARG, "Unknown converted naming field.");
      endfor
      $root:_validate_noun_grammar(['kind -> `settings['kind] ! E_RANGE => target.noun_kind',
        'number -> `settings['number] ! E_RANGE => target.noun_number',
        'article -> `settings['article] ! E_RANGE => target.indefinite_article']);
    endfor
    for record in (records)
      const target = record['object];
      const fields = ['kind -> "noun_kind", 'number -> "noun_number", 'article -> "indefinite_article"];
      for field in (mapkeys(record['settings]))
        target.(fields[field]) = record['settings][field];
      endfor
      if (record['letter])
        target.subject = record['subject];
        target.name = "letter";
      endif
    endfor
    return length(records);
  endmethod
endobject
