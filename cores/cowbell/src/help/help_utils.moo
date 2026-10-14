object HELP_UTILS [
  import_export_id -> "help_utils",
  import_export_hierarchy -> {"help"}
]
  name: "Help Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Help utilities for documentation extraction and lookup. Provides verbs for extracting verb docstrings, object documentation, and parsing help references.";

  method annotate_prose owner: ARCH_WIZARD
    "Resolve code mentions in parsed help blocks; never scan markup or enter interactive labels.";
    const {blocks, viewer} = args;
    set_task_perms(caller_perms());
    $document:validate($document:mk(blocks));
    valid(viewer) || return $format.block:mk(@blocks);
    const environment = {@viewer:command_environment(), @viewer.contents,
      @`viewer.location.contents ! E_INVIND => {}'};
    let annotated = {};
    let resolved = [];
    for block in (blocks)
      const {value, updated} = this:_annotate_node(block, 'block, viewer, environment, resolved);
      annotated = {@annotated, value};
      resolved = updated;
    endfor
    return $format.block:mk(@annotated);
  endmethod

  method _annotate_node owner: ARCH_WIZARD
    "Walk known content fields, leaving code blocks and existing interactive labels opaque.";
    let {value, expected, viewer, environment, resolved} = args;
    set_task_perms(caller_perms());
    const node = expected in {'inline, 'block};
    const kind = node ? value.kind | expected;
    kind in {'link, 'reference, 'code_block} && return {value, resolved};
    if (kind == 'inline_code)
      return this:_annotate_command(value, viewer, environment, resolved);
    endif
    const shape = $document_schema:_shape(kind, "help");
    let fields = node ? flyslots(value) | value;
    node && (fields = mapdelete(fields, 'kind));
    const contents_key = shape['contents];
    if (typeof(contents_key) == TYPE_SYM)
      fields[contents_key] = flycontents(value);
    endif
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      if (!maphaskey(fields, key))
        continue;
      endif
      let values = {};
      for entry in (fields[key])
        if (cardinality == 'matrix_optional)
          let row = {};
          for cell in (entry)
            const {annotated, updated} = this:_annotate_node(cell, child_kind, viewer, environment, resolved);
            row = {@row, annotated};
            resolved = updated;
          endfor
          values = {@values, row};
        else
          const {annotated, updated} = this:_annotate_node(entry, child_kind, viewer, environment, resolved);
          values = {@values, annotated};
          resolved = updated;
        endif
      endfor
      fields[key] = values;
    endfor
    !node && return {fields, resolved};
    let contents = {};
    if (typeof(contents_key) == TYPE_SYM)
      contents = fields[contents_key];
      fields = mapdelete(fields, contents_key);
    endif
    return {$document_template:mk(kind, contents, fields), resolved};
  endmethod

  method _annotate_command owner: ARCH_WIZARD
    "Resolve eligible inline code in the viewer's command environment without executing it.";
    let {code, viewer, environment, resolved} = args;
    set_task_perms(caller_perms());
    const label = code.text;
    (index(label, "\n") || index(label, "`")) && return {code, resolved};
    const words = label:words_list();
    const name = length(words) ? words[1] | "";
    if (!maphaskey(resolved, name))
      resolved[name] = {};
      for receiver in (environment)
        const definer = `receiver:find_verb_definer(name) ! E_VERBNF, E_PERM => $nothing';
        if (!valid(definer))
          continue;
        endif
        const info = verb_info(definer, name);
        const argspec = verb_args(definer, name);
        if (index(info[2], "r") && argspec != {"this", "none", "this"})
          resolved[name] = {receiver, definer, argspec};
          break;
        endif
      endfor
    endif
    const reference = resolved[name];
    !length(reference) && return {code, resolved};
    let invocation;
    if (index(label, "<"))
      invocation = this:command_usage(label, viewer, reference[3]);
      "Examples with more than two input fields remain code; the interaction protocol has two slots.";
      typeof(invocation) == TYPE_FLYWEIGHT || return {code, resolved};
    elseif (index(label, "["))
      return {code, resolved};
    else
      invocation = $format.annotation:command(label, code);
    endif
    const fragment = $obj_utils:command_with_source(invocation, reference[1], name, reference[2], viewer);
    return {fragment, resolved};
  endmethod

  method command_usage owner: ARCH_WIZARD
    "Use an article's authored placeholders without binding an incidental nearby receiver.";
    set_task_perms(caller_perms());
    const {usage, viewer, argspec} = args;
    let template = usage;
    let fields = [];
    for slot in ({"dobj", "iobj"})
      const opening = index(template, "<");
      if (!opening)
        break;
      endif
      const closing = index(template[opening..$], ">");
      closing > 1 || return usage;
      const last = opening + closing - 1;
      const label = template[opening + 1..last - 1];
      template = template[1..opening - 1] + "{" + slot + "}" + template[last + 1..$];
      fields[slot] = ["label" -> label:capitalize(), "expectedKind" -> "text"];
    endfor
    index(template, "<") && return usage;
    if (!fields)
      return $format.annotation:command(usage, $format.code:inline(usage));
    endif
    "Unconstrained verbs consume free text; a parser slot alone does not imply an object.";
    if (argspec == {"any", "any", "any"})
      return $format.annotation:command_template(template, fields, $format.code:inline(usage));
    endif
    const parsed = parse_command(strsub(strsub(template, "{dobj}", "#-2"), "{iobj}", "#-3"), {}, false);
    for slot in (mapkeys(fields))
      const marker = slot == "dobj" ? "#-2" | "#-3";
      if (parsed['dobjstr] == marker || parsed['iobjstr] == marker)
        fields[slot]["expectedKind"] = "object";
        fields[slot]["suggestions"] = ["provider" -> $url_utils:to_curie_str(viewer), "source" -> "nearby"];
      endif
    endfor
    return $format.annotation:command_template(template, fields, $format.code:inline(usage));
  endmethod

  method extract_verb_documentation owner: ARCH_WIZARD
    "Extract documentation from a verb's code. Returns list of all comment lines (string literals ending with ;) from the start of the verb until the first non-comment line.";
    "Args: {verb_location, verb_name}";
    "Returns: documentation list (empty list if no comments found)";
    set_task_perms(caller_perms());
    {verb_location, verb_name} = args;
    "Get the verb code with line numbers";
    code_lines = `verb_code(verb_location, verb_name, false, true) ! E_VERBNF => {}';
    if (typeof(code_lines) != TYPE_LIST || length(code_lines) == 0)
      return {};
    endif
    "Collect comment lines from the start of the verb";
    doc_lines = {};
    for line in (code_lines)
      "A comment line is a string literal: starts with \" and ends with \"; (or just ;)";
      line_trimmed = line:trim();
      if (line_trimmed && line_trimmed[1] == "\"" && (line_trimmed[length(line_trimmed)] == ";" || line_trimmed[length(line_trimmed) - 1..length(line_trimmed)] == "\";"))
        "This is a comment line, add it";
        "Remove the quotes and semicolon";
        doc_line = line_trimmed[2..length(line_trimmed) - 2];
        doc_lines = {@doc_lines, doc_line};
      else
        "Hit a non-comment line, stop collecting";
        break;
      endif
    endfor
    return doc_lines;
  endmethod

  method get_object_documentation owner: ARCH_WIZARD
    "Get documentation for an object. Returns the object_documentation property if it exists and is not 0, otherwise empty string.";
    "Args: {object}";
    "Returns: documentation content (string or list)";
    set_task_perms(caller_perms());
    {target} = args;
    if (!valid(target))
      return "";
    endif
    doc = `target.object_documentation ! E_PROPNF => 0';
    if (!doc || doc == 0)
      return "";
    endif
    return doc;
  endmethod

  method property_documentation owner: ARCH_WIZARD
    "Get documentation for a property. Looks for {prop_name}_prop_doc property on the object.";
    "Args: {object, property_name}";
    "Returns: documentation list (empty list if not found)";
    set_task_perms(caller_perms());
    {target, prop_name} = args;
    if (!valid(target))
      return {};
    endif
    doc_prop_name = prop_name + "_prop_doc";
    props = properties(target);
    if (!(doc_prop_name in props))
      return {};
    endif
    doc = `target.(doc_prop_name) ! E_PROPNF => 0';
    if (!doc || doc == 0)
      return {};
    endif
    "If doc is already a list, return it; if it's a string, wrap it";
    if (typeof(doc) == TYPE_LIST)
      return doc;
    else
      return {doc};
    endif
  endmethod

  method format_documentation_display owner: ARCH_WIZARD
    "Construct a heading and documentation blocks. Source lines become literal prose; explicit blocks retain their structure.";
    set_task_perms(caller_perms());
    let {title, doc_text} = args;
    if (typeof(doc_text) == TYPE_LIST)
      doc_text = doc_text:join("\n");
    endif
    if ((typeof(doc_text) == TYPE_STR && !length(doc_text))
        || (typeof(doc_text) == TYPE_BOOL && !doc_text))
      doc_text = "(No documentation available)";
    endif
    return $format.block:mk($format.title:mk(title), doc_text);
  endmethod

  method display_location_context owner: ARCH_WIZARD
    "Display location, area, inventory, and nearby objects. Returns $format flyweight.";
    "Args: {player}";
    "Returns: $format.block with location context as formatted content";
    set_task_perms(caller_perms());
    {player_obj} = args;
    content = {};
    location = player_obj.location;
    area = location.location;
    location_name = `location:noun_phrase('definite, player_obj) ! ANY => "somewhere"';
    location_desc = {"You are in ", $format.annotation:object(location, location_name)};
    if (valid(area))
      area_name = `area:noun_phrase('definite, player_obj) ! ANY => "somewhere"';
      location_desc = {@location_desc, " in ", $format.annotation:object(area, area_name)};
    endif
    content = {@content, $format.paragraph:mk(@location_desc)};
    "Inventory";
    inventory = player_obj:contents();
    if (inventory && length(inventory) > 0)
      inv_names = {};
      for item in (inventory)
        inv_names = {@inv_names, @length(inv_names) ? {", "} | {}, $sub:reference(item, 'listing)};
      endfor
      content = {@content, $format.paragraph:mk("You are carrying ", @inv_names, ".")};
    endif
    "Ways to go (exits)";
    if (valid(area) && respond_to(area, 'passages_from))
      passages = `area:passages_from(location) ! ANY => {}';
      if (passages && length(passages) > 0)
        exit_labels = {};
        for passage in (passages)
          is_open = `passage.is_open ! ANY => false';
          if (!is_open)
            continue;
          endif
          info = `passage:side_info_for(location) ! ANY => {}';
          if (length(info) == 0)
            continue;
          endif
          {label, description, ambient} = info;
          if (label)
            exit_labels = {@exit_labels, @length(exit_labels) ? {", "} | {}, $format.annotation:exit(location, label)};
          endif
        endfor
        if (exit_labels && length(exit_labels) > 0)
          content = {@content, $format.paragraph:mk("You can go ", @exit_labels, ".")};
        endif
      endif
    endif
    "Nearby objects (excluding location itself)";
    nearby = location:contents();
    nearby_items = {};
    for nearby_obj in (nearby)
      if (nearby_obj != player_obj && nearby_obj != location)
        nearby_items = {@nearby_items, nearby_obj};
      endif
    endfor
    if (nearby_items && length(nearby_items) > 0)
      nearby_names = {};
      for item in (nearby_items)
        nearby_names = {@nearby_names, @length(nearby_names) ? {", "} | {}, $sub:reference(item, 'listing)};
      endfor
      content = {@content, $format.paragraph:mk("Around you: ", @nearby_names, ".")};
    endif
    return $format.block:mk(@content);
  endmethod

  method verb_help_from_hint owner: ARCH_WIZARD
    "Extract help topic from a verb's HINT tag.";
    "Args: (definer, verb_name, ?category) -> $help flyweight or 0";
    {definer, verb_name, ?category = 'command} = args;
    code = `verb_code(definer, verb_name) ! E_VERBNF => {}';
    if (!code || length(code) == 0)
      return 0;
    endif
    first_line = code[1]:trim();
    "Check if it's a string literal starting with HINT:";
    if (!first_line:starts_with("\"HINT:"))
      return 0;
    endif
    "Extract content between quotes";
    end_quote = rindex(first_line, "\"");
    if (end_quote <= 6)
      return 0;
    endif
    hint_content = first_line[7..end_quote - 1]:trim();
    "Parse hint: '<usage> -- <description>' or just '<description>'";
    dash_pos = index(hint_content, " -- ");
    if (dash_pos > 0)
      usage = hint_content[1..dash_pos - 1]:trim();
      description = hint_content[dash_pos + 4..$]:trim();
      content = $format.paragraph:mk("Usage: ",
        this:command_usage(verb_name + " " + usage, player, verb_args(definer, verb_name)));
    else
      description = hint_content;
      content = "";
    endif
    return $help:mk(verb_name, description, content, {}, category, {});
  endmethod
endobject
