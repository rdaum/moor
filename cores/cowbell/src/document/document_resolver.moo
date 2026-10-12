object DOCUMENT_RESOLVER [
  import_export_id -> "document_resolver",
  import_export_hierarchy -> {"document"}
]
  name: "Document Resolution"
  parent: ROOT
  owner: HACKER
  readable: true

  method resolve owner: ARCH_WIZARD
    "Resolve block templates for a viewer under the caller's authority, then validate the document.";
    "An absent viewer is false. Facts are captured values; a supplied principal is never accepted.";
    set_task_perms(caller_perms());
    const {blocks, viewer, facts} = args;
    (typeof(viewer) == TYPE_BOOL && !viewer)
      || (typeof(viewer) == TYPE_OBJ && valid(viewer)) || raise(E_INVARG, "Invalid viewer");
    typeof(facts) == TYPE_MAP || raise(E_TYPE, "Event facts must be a map");
    const context = ['viewer -> viewer, 'principal -> caller_perms(), 'event -> facts];
    const {resolved, state} = $document_resolver:_sequence(
      blocks, 'block, context, "document.blocks", 1, ['expansions -> 0]);
    const document = $document:mk(resolved);
    $document:validate(document);
    return document;
  endmethod

  method _sequence owner: ARCH_WIZARD
    "Expand a typed sequence while carrying one resource budget through all branches.";
    set_task_perms(caller_perms());
    let {values, expected, context, path, depth, state} = args;
    context['principal] == caller_perms() || raise(E_PERM, "Resolution principal mismatch");
    typeof(values) == TYPE_LIST || raise(E_INVARG, path + ": expected a sequence");
    length(values) <= 8192 || raise(E_QUOTA, path + ": too many template entries");
    let result = {};
    for i in [1..length(values)]
      const {nodes, next_state} = $document_resolver:_walk(values[i], expected, context,
        path + "[" + tostr(i - 1) + "]", depth, state);
      state = next_state;
      result = {@result, @nodes};
      length(result) <= 8192 || raise(E_QUOTA, path + ": too many resolved entries");
    endfor
    return {result, state};
  endmethod

  method _walk owner: ARCH_WIZARD
    "Resolve known structure and explicit hooks, using one MOO activation per template level.";
    set_task_perms(caller_perms());
    let {value, expected, context, path, depth, state} = args;
    context['principal] == caller_perms() || raise(E_PERM, "Resolution principal mismatch");
    depth <= 32 || raise(E_QUOTA, path + ": template nesting exceeds 32 levels");
    state['expansions] = state['expansions] + 1;
    state['expansions] <= 8192 || raise(E_QUOTA, path + ": too many template expansions");
    let fields = [];
    let kind = expected;
    let contents = {};
    let expansion = false;
    if (expected in {'inline, 'block})
      typeof(value) == TYPE_FLYWEIGHT || raise(E_INVARG, path + ": expected content node");
      if (value.delegate == $sub)
        expected == 'inline || raise(E_INVARG, path + ": substitution requires inline content");
        expansion = value:resolve_inline(context);
      else
        value.delegate in {$document_node, $document_template}
          || raise(E_INVARG, path + ": use an explicit producer constructor for custom content");
        fields = flyslots(value);
        maphaskey(fields, 'kind) || raise(E_INVARG, path + ": missing node kind");
        kind = fields['kind];
        contents = flycontents(value);
        if (value.delegate == $document_template && kind in {'inline, 'capitalize, 'blocks})
          (kind in {'inline, 'capitalize} && expected == 'inline)
            || (kind == 'blocks && expected == 'block)
            || raise(E_INVARG, path + ": wrong template context");
          length(fields) == 1 || raise(E_INVARG, path + ": unexpected group field");
          expansion = contents;
        elseif (value.delegate == $document_template && kind in {'producer_inline, 'producer_blocks})
          const role = kind == 'producer_inline ? 'inline | 'block;
          role == expected || raise(E_INVARG, path + ": wrong producer context");
          length(fields) == 2 && maphaskey(fields, 'producer) && !length(contents)
            || raise(E_INVARG, path + ": invalid producer fields");
          const producer = fields['producer];
          typeof(producer) in {TYPE_OBJ, TYPE_FLYWEIGHT}
            || raise(E_INVARG, path + ": invalid content producer");
          try
            expansion = role == 'inline
              ? producer:resolve_inline(context) | producer:resolve_blocks(context);
          except error (ANY)
            raise(error[1], path + ": producer failed: " + error[2]);
          endtry
          typeof(expansion) == TYPE_LIST || raise(E_INVARG, path + ": producer must return a sequence");
        endif
      endif
    else
      typeof(value) == TYPE_MAP || raise(E_INVARG, path + ": expected structural record");
      fields = value;
    endif
    if (typeof(expansion) == TYPE_LIST)
      length(expansion) <= 8192 || raise(E_QUOTA, path + ": too many expanded entries");
      let result = {};
      for i in [1..length(expansion)]
        const {nodes, next_state} = $document_resolver:_walk(expansion[i], expected,
          context, path + ".expanded[" + tostr(i - 1) + "]", depth + 1, state);
        state = next_state;
        result = {@result, @nodes};
      endfor
      kind == 'capitalize && (result = $document_resolver:_capitalize(result)[1]);
      return {result, state};
    endif
    const shape = $document_schema:_shape(kind, path);
    if (expected in {'inline, 'block})
      shape['group] == expected || raise(E_INVARG, path + ": wrong node context");
      if (typeof(shape['contents]) == TYPE_SYM)
        !maphaskey(fields, shape['contents]) || raise(E_INVARG, path + ": duplicate children");
        fields[shape['contents]] = contents;
      else
        length(contents) == 0 || raise(E_INVARG, path + ": node cannot contain children");
      endif
    endif
    let resolved_contents = {};
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      const child_path = path + "." + tostr(key);
      if (!maphaskey(fields, key))
        cardinality == 'matrix_optional || raise(E_INVARG, child_path + ": missing field");
        continue;
      endif
      if (cardinality == 'matrix_optional && typeof(fields[key]) == TYPE_BOOL && !fields[key])
        fields = mapdelete(fields, key);
        continue;
      endif
      typeof(fields[key]) == TYPE_LIST || raise(E_INVARG, child_path + ": expected sequence");
      length(fields[key]) <= 8192 || raise(E_QUOTA, child_path + ": too many entries");
      let children = {};
      for i in [1..length(fields[key])]
        const entry_path = child_path + "[" + tostr(i - 1) + "]";
        const sequence = cardinality == 'matrix_optional ? fields[key][i] | {fields[key][i]};
        typeof(sequence) == TYPE_LIST || raise(E_INVARG, entry_path + ": expected inline sequence");
        length(sequence) <= 8192 || raise(E_QUOTA, entry_path + ": too many entries");
        let resolved = {};
        for j in [1..length(sequence)]
          const item_path = cardinality == 'matrix_optional
            ? entry_path + "[" + tostr(j - 1) + "]" | entry_path;
          const {nodes, next_state} = $document_resolver:_walk(sequence[j], child_kind,
            context, item_path, depth + 1, state);
          state = next_state;
          resolved = {@resolved, @nodes};
        endfor
        children = cardinality == 'matrix_optional ? {@children, resolved} | {@children, @resolved};
      endfor
      if (key == shape['contents])
        resolved_contents = children;
        fields = mapdelete(fields, key);
      else
        fields[key] = children;
      endif
    endfor
    if (shape['group] == 'record)
      return {{fields}, state};
    endif
    return {{toflyweight($document_node, fields, resolved_contents)}, state};
  endmethod

  method binding owner: HACKER
    "Read one captured event binding. Values are tagged as object, text, or content.";
    const {context, name} = args;
    const aliases = ['n -> 'actor, 'nc -> 'actor, 'd -> 'dobj, 'dc -> 'dobj,
      'i -> 'iobj, 'ic -> 'iobj, 't -> 'this_obj, 'tc -> 'this_obj,
      'this -> 'this_obj, 'l -> 'location, 'lc -> 'location];
    const key = maphaskey(aliases, name) ? aliases[name] | name;
    const facts = context['event];
    if (key in {'actor, 'dobj, 'iobj, 'this_obj, 'location})
      maphaskey(facts, key) || raise(E_INVARG, "Missing event binding: " + tostr(key));
      return ['kind -> 'object, 'value -> facts[key]];
    endif
    const bindings = maphaskey(facts, 'bindings) ? facts['bindings] | [];
    maphaskey(bindings, key) || raise(E_INVARG, "Missing event binding: " + tostr(key));
    return bindings[key];
  endmethod

  method _capitalize owner: HACKER
    "Capitalize the first nonempty resolved inline text while retaining reference and style structure.";
    const {nodes} = args;
    let result = {};
    let changed = false;
    for node in (nodes)
      let value = node;
      if (!changed && node.kind in {'text, 'inline_code} && length(node.text))
        value.text = node.text:capitalize();
        changed = true;
      elseif (!changed && length(flycontents(node)))
        const {children, capitalized} = $document_resolver:_capitalize(flycontents(node));
        value = toflyweight($document_node, flyslots(node), children);
        changed = capitalized;
      endif
      result = {@result, value};
    endfor
    return {result, changed};
  endmethod
endobject
