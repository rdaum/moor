object DOCUMENT_SCHEMA [
  import_export_id -> "document_schema",
  import_export_hierarchy -> {"document"}
]
  name: "Document Validation and Encoding"
  parent: ROOT
  owner: HACKER
  readable: true

  method _shape owner: HACKER
    "Return the fixed field schema. Child fields are ordered by reading order.";
    const {kind, path} = args;
    const shapes = [
      'document -> ['group -> 'root, 'required -> ['version -> 'version],
        'optional -> [], 'children -> {{'blocks, 'block, 'many}},
        'contents -> 'blocks],
      'text -> ['group -> 'inline, 'required -> ['text -> 'text],
        'optional -> [], 'children -> {},
        'contents -> false],
      'inline_code -> ['group -> 'inline, 'required -> ['text -> 'string],
        'optional -> [], 'children -> {},
        'contents -> false],
      'soft_break -> ['group -> 'inline, 'required -> [],
        'optional -> [], 'children -> {},
        'contents -> false],
      'hard_break -> ['group -> 'inline, 'required -> [],
        'optional -> [], 'children -> {},
        'contents -> false],
      'emphasis -> ['group -> 'inline, 'required -> [],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'strong -> ['group -> 'inline, 'required -> [],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'strike -> ['group -> 'inline, 'required -> [],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'link -> ['group -> 'inline, 'required -> ['url -> 'url],
        'optional -> ['title -> 'string], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'reference -> ['group -> 'inline, 'required -> ['descriptor -> 'descriptor],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'image -> ['group -> 'inline, 'required -> ['src -> 'image_url, 'alt -> 'text],
        'optional -> ['title -> 'string], 'children -> {},
        'contents -> false],
      'paragraph -> ['group -> 'block, 'required -> [],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'heading -> ['group -> 'block, 'required -> ['level -> 'level],
        'optional -> [], 'children -> {{'children, 'inline, 'many}},
        'contents -> 'children],
      'block_quote -> ['group -> 'block, 'required -> [],
        'optional -> [], 'children -> {{'children, 'block, 'many}},
        'contents -> 'children],
      'code_block -> ['group -> 'block, 'required -> ['text -> 'string],
        'optional -> ['language -> 'language, 'first_line -> 'positive], 'children -> {},
        'contents -> false],
      'thematic_break -> ['group -> 'block, 'required -> [],
        'optional -> [], 'children -> {},
        'contents -> false],
      'list -> ['group -> 'block, 'required -> ['style -> 'list_style],
        'optional -> ['start -> 'integer], 'children -> {{'items, 'list_item, 'many}},
        'contents -> 'items],
      'list_item -> ['group -> 'record, 'required -> [],
        'optional -> ['checked -> 'boolean], 'children -> {{'blocks, 'block, 'many}},
        'contents -> false],
      'definition_list -> ['group -> 'block, 'required -> [],
        'optional -> [], 'children -> {{'items, 'definition_item, 'many}},
        'contents -> 'items],
      'definition_item -> ['group -> 'record, 'required -> [],
        'optional -> [], 'children -> {{'term, 'inline, 'many}, {'definition, 'block, 'many}},
        'contents -> false],
      'table -> ['group -> 'block, 'required -> [],
        'optional -> [], 'children -> {{'caption, 'inline, 'many},
          {'columns, 'column, 'many}, {'header, 'inline, 'matrix_optional},
          {'rows, 'row, 'many}},
        'contents -> 'rows],
      'column -> ['group -> 'record, 'required -> ['align -> 'align],
        'optional -> [], 'children -> {},
        'contents -> false],
      'row -> ['group -> 'record, 'required -> [],
        'optional -> [], 'children -> {{'cells, 'cell, 'many}},
        'contents -> false],
      'cell -> ['group -> 'record, 'required -> [],
        'optional -> [], 'children -> {{'blocks, 'block, 'many}},
        'contents -> false]
    ];
    maphaskey(shapes, kind) || raise(E_INVARG, path + ": unknown node kind");
    return shapes[kind];
  endmethod
  method encode owner: HACKER
    "Validate canonical flyweights and return inert version 1 metadata. No callbacks or suspension.";
    const {document} = args;
    return $document_schema:_checked(document, false, false)['encoded];
  endmethod

  method decode owner: HACKER
    "Reject malformed metadata before constructing any canonical document value.";
    const {encoded} = args;
    const checked = $document_schema:_checked(encoded, true, false);
    return $document_schema:_decode(checked['encoded], 'document);
  endmethod

  method prepare owner: HACKER
    "Remove invalid or excess interactions once, returning explicit path diagnostics.";
    const {document} = args;
    const checked = $document_schema:_checked(document, false, true);
    return ['document -> $document_schema:_decode(checked['encoded], 'document),
      'encoded -> checked['encoded], 'diagnostics -> checked['diagnostics]];
  endmethod

  method _checked owner: HACKER
    "Bound and validate the full tree, then enforce the compact UTF-8 metadata limit.";
    const {value, wire, finalize} = args;
    const state = ['nodes -> 0, 'references -> 0, 'bytes -> 0, 'diagnostics -> {}];
    const {encoded, finished, splice} =
      $document_schema:_walk(value, 'document, "document", 0, false, state, wire, finalize);
    length(binary_from_str(generate_json(encoded))) <= 262144
      || raise(E_QUOTA, "document: encoded document exceeds 256 KiB");
    return ['encoded -> encoded, 'diagnostics -> finished['diagnostics]];
  endmethod

  method _walk owner: HACKER
    "Encode one node or record. Carry resource counts and interaction context through its children.";
    let {value, expected, path, depth, interactive, state, wire, finalize} = args;
    depth <= 32 || raise(E_QUOTA, path + ": document nesting exceeds 32 levels");
    if (depth > 0)
      state['nodes] = state['nodes] + 1;
      state['nodes] <= 8192 || raise(E_QUOTA, path + ": document exceeds 8192 records");
    endif
    let kind = expected;
    let fields = [];
    let contents = {};
    if (wire)
      fields = $document_schema:_wire_fields(value, path);
      if (expected in {'inline, 'block})
        maphaskey(fields, 'kind) || raise(E_INVARG, path + ": missing kind");
        const name = fields['kind];
        typeof(name) == TYPE_STR || raise(E_INVARG, path + ".kind: expected string");
        kind = tosym(name);
        strcmp(name, tostr(kind)) == 0 || raise(E_INVARG, path + ".kind: invalid spelling");
      endif
    elseif (expected in {'document, 'inline, 'block})
      typeof(value) == TYPE_FLYWEIGHT || raise(E_INVARG, path + ": expected canonical flyweight");
      const delegate = expected == 'document ? $document | $document_node;
      value.delegate == delegate || raise(E_INVARG, path + ": wrong document delegate");
      fields = flyslots(value);
      contents = flycontents(value);
      if (expected != 'document)
        maphaskey(fields, 'kind) || raise(E_INVARG, path + ": missing kind");
        kind = fields['kind];
        typeof(kind) == TYPE_SYM || raise(E_INVARG, path + ".kind: expected symbol");
      endif
    else
      typeof(value) == TYPE_MAP || raise(E_INVARG, path + ": expected record");
      fields = value;
    endif
    match(tostr(kind), "^[a-z_]+$", 1) || raise(E_INVARG, path + ".kind: invalid spelling");
    const shape = $document_schema:_shape(kind, path);
    if (expected in {'inline, 'block})
      shape['group] == expected || raise(E_INVARG, path + ": expected " + tostr(expected));
    endif
    const is_node = shape['group] in {'inline, 'block};
    if (!wire && expected in {'document, 'inline, 'block})
      if (typeof(shape['contents]) == TYPE_SYM)
        !maphaskey(fields, shape['contents])
          || raise(E_INVARG, path + ": children must occupy flyweight contents");
        fields[shape['contents]] = contents;
      else
        length(contents) == 0 || raise(E_INVARG, path + ": node cannot contain children");
      endif
    endif
    let allowed = {@mapkeys(shape['required]), @mapkeys(shape['optional])};
    for child in (shape['children])
      allowed = {@allowed, child[1]};
    endfor
    is_node && (allowed = {@allowed, 'kind});
    for key in (mapkeys(fields))
      (typeof(key) == TYPE_SYM && key in allowed && match(tostr(key), "^[a-z_]+$", 1))
        || raise(E_INVARG, path + ": unknown field " + toliteral(key));
    endfor
    if (kind in {'link, 'reference})
      !interactive || raise(E_INVARG, path + ": interactive labels cannot nest");
      interactive = true;
    endif
    let encoded = is_node ? ["kind" -> tostr(kind)] | [];
    let drop_reference = false;
    for key in ({@mapkeys(shape['required]), @mapkeys(shape['optional])})
      const required = maphaskey(shape['required], key);
      if (!maphaskey(fields, key))
        !required || raise(E_INVARG, path + "." + tostr(key) + ": missing field");
        continue;
      endif
      const rule = required ? shape['required][key] | shape['optional][key];
      if (!wire && !required && rule != 'boolean && typeof(fields[key]) == TYPE_BOOL && !fields[key])
        continue;
      endif
      const field_path = path + "." + tostr(key);
      if (rule == 'descriptor)
        try
          encoded[tostr(key)] = $document_schema:_descriptor(fields[key], field_path, wire);
        except error (E_INVARG)
          finalize || raise(error[1], error[2]);
          drop_reference = true;
          state['diagnostics] = {@state['diagnostics], error[2]};
        endtry
      else
        const scalar = $document_schema:_scalar(fields[key], rule, field_path, wire);
        encoded[tostr(key)] = scalar;
        if (typeof(scalar) == TYPE_STR)
          state['bytes] = state['bytes] + length(binary_from_str(scalar));
          state['bytes] <= 262144 || raise(E_QUOTA, field_path + ": document text exceeds 256 KiB");
        endif
      endif
    endfor
    if (kind == 'reference && !drop_reference)
      if (state['references] >= 512)
        finalize || raise(E_QUOTA, path + ": more than 512 references; finalize the document");
        drop_reference = true;
        state['diagnostics] = {@state['diagnostics], path + ": reference budget exceeded"};
      else
        state['references] = state['references] + 1;
      endif
    endif
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      const child_path = path + "." + tostr(key);
      if (!maphaskey(fields, key))
        cardinality == 'matrix_optional || raise(E_INVARG, child_path + ": missing field");
        continue;
      endif
      if (!wire && cardinality == 'matrix_optional && typeof(fields[key]) == TYPE_BOOL && !fields[key])
        continue;
      endif
      typeof(fields[key]) == TYPE_LIST || raise(E_INVARG, child_path + ": expected list");
      length(fields[key]) <= 8192 || raise(E_QUOTA, child_path + ": too many entries");
      let children = {};
      for i in [1..length(fields[key])]
        const entry_path = child_path + "[" + tostr(i - 1) + "]";
        let sequence = {fields[key][i]};
        if (cardinality == 'matrix_optional)
          typeof(fields[key][i]) == TYPE_LIST || raise(E_INVARG, entry_path + ": expected inline list");
          sequence = fields[key][i];
          length(sequence) <= 8192 || raise(E_QUOTA, entry_path + ": too many entries");
        endif
        let encoded_sequence = {};
        for j in [1..length(sequence)]
          const entry = sequence[j];
          const node_path = cardinality == 'matrix_optional
            ? entry_path + "[" + tostr(j - 1) + "]" | entry_path;
          const {result, next_state, splice} = $document_schema:_walk(
            entry, child_kind, node_path, depth + 1, interactive, state, wire, finalize);
          state = next_state;
          encoded_sequence = splice ? {@encoded_sequence, @result} | {@encoded_sequence, result};
        endfor
        children = cardinality == 'matrix_optional
          ? {@children, encoded_sequence} | {@children, @encoded_sequence};
      endfor
      encoded[tostr(key)] = children;
    endfor
    if (kind == 'text)
      (!index(encoded["text"], "\n") && !index(encoded["text"], "\r"))
        || raise(E_INVARG, path + ".text: use break nodes for newlines");
    elseif (kind == 'list)
      if (encoded["style"] == "bullet")
        !maphaskey(encoded, "start") || raise(E_INVARG, path + ".start: bullet lists have no start");
      endif
    elseif (kind == 'table)
      const width = length(encoded["columns"]);
      width > 0 || raise(E_INVARG, path + ".columns: table needs at least one column");
      if (maphaskey(encoded, "header"))
        length(encoded["header"]) == width || raise(E_INVARG, path + ".header: wrong column count");
      endif
      for i in [1..length(encoded["rows"])]
        length(encoded["rows"][i]["cells"]) == width
          || raise(E_INVARG, path + ".rows[" + tostr(i - 1) + "].cells: wrong column count");
      endfor
    endif
    return {drop_reference ? encoded["children"] | encoded, state, drop_reference};
  endmethod

  method _wire_fields owner: HACKER
    "Convert exact string field names to symbols, rejecting aliases and case variants.";
    const {value, path} = args;
    typeof(value) == TYPE_MAP || raise(E_INVARG, path + ": expected metadata map");
    let fields = [];
    for key in (mapkeys(value))
      typeof(key) == TYPE_STR || raise(E_INVARG, path + ": metadata keys must be strings");
      match(key, "^[a-z_]+$", 1) || raise(E_INVARG, path + ": invalid field spelling");
      const symbol = tosym(key);
      fields[symbol] = value[key];
    endfor
    return fields;
  endmethod

  method _scalar owner: HACKER
    "Validate one scalar and encode enums as strings. URLs never contain whitespace or controls.";
    const {value, rule, path, wire} = args;
    if (rule in {'version, 'level, 'positive, 'integer})
      typeof(value) == TYPE_INT || raise(E_INVARG, path + ": expected integer");
      if (rule == 'version)
        value == 1 || raise(E_INVARG, path + ": unsupported document version");
      elseif (rule == 'level)
        (value >= 1 && value <= 6) || raise(E_INVARG, path + ": heading level must be 1 through 6");
      else
        (value >= -9007199254740991 && value <= 9007199254740991)
          || raise(E_INVARG, path + ": integer is outside the exact JSON range");
        if (rule == 'positive)
          value > 0 || raise(E_INVARG, path + ": expected positive integer");
        endif
      endif
      return value;
    elseif (rule == 'boolean)
      typeof(value) == TYPE_BOOL || raise(E_INVARG, path + ": expected boolean");
      return value;
    elseif (rule in {'align, 'list_style})
      const values = rule == 'align ? {'start, 'center, 'end} | {'bullet, 'numbered};
      typeof(value) == (wire ? TYPE_STR | TYPE_SYM)
        || raise(E_INVARG, path + ": invalid enum type");
      for allowed in (values)
        if (strcmp(tostr(value), tostr(allowed)) == 0)
          return tostr(allowed);
        endif
      endfor
      raise(E_INVARG, path + ": invalid enum value");
    endif
    typeof(value) == TYPE_STR || raise(E_INVARG, path + ": expected string");
    length(binary_from_str(value)) <= 262144 || raise(E_QUOTA, path + ": string exceeds 256 KiB");
    if (rule == 'language)
      (!length(value) || match(value, "^[A-Za-z0-9_.+-]+$", 1))
        || raise(E_INVARG, path + ": invalid code language");
    elseif (rule in {'url, 'image_url})
      const safe_scheme = index(value, "https://") == 1 || index(value, "http://") == 1;
      (safe_scheme || (rule == 'url && index(value, "mailto:") == 1))
        || raise(E_INVARG, path + ": unsupported URL scheme");
      const bytes = binary_from_str(value);
      for i in [1..length(bytes)]
        (bytes[i] > 32 && bytes[i] != 127)
          || raise(E_INVARG, path + ": URL contains whitespace or controls");
      endfor
    endif
    return value;
  endmethod

  method _decode owner: HACKER
    "Reconstruct a value from already validated metadata, without resolving object identities.";
    const {encoded, expected} = args;
    const kind = expected in {'inline, 'block} ? tosym(encoded["kind"]) | expected;
    const shape = $document_schema:_shape(kind, "document");
    let fields = [];
    let contents = {};
    for key in ({@mapkeys(shape['required]), @mapkeys(shape['optional])})
      if (!maphaskey(encoded, tostr(key)))
        continue;
      endif
      const rule = maphaskey(shape['required], key) ? shape['required][key] | shape['optional][key];
      const value = encoded[tostr(key)];
      fields[key] = rule in {'align, 'list_style} ? tosym(value) | value;
      if (rule == 'descriptor)
        fields[key] = $document_schema:_descriptor_from_wire(value);
      endif
    endfor
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      if (!maphaskey(encoded, tostr(key)))
        continue;
      endif
      let children = {};
      for value in (encoded[tostr(key)])
        if (cardinality == 'matrix_optional)
          children = {@children, {$document_schema:_decode(entry, child_kind) for entry in (value)}};
        else
          children = {@children, $document_schema:_decode(value, child_kind)};
        endif
      endfor
      if (key == shape['contents])
        contents = children;
      else
        fields[key] = children;
      endif
    endfor
    if (kind == 'document)
      return toflyweight($document, fields, contents);
    elseif (shape['group] == 'record)
      return fields;
    endif
    fields['kind] = kind;
    return toflyweight($document_node, fields, contents);
  endmethod
  method _descriptor owner: HACKER
    "Validate the interaction protocol and return an inert string-keyed descriptor.";
    const {value, path, wire} = args;
    const fields = $document_schema:_interaction_fields(value, path, wire);
    maphaskey(fields, 'kind) || raise(E_INVARG, path + ": missing interaction kind");
    const kind = $document_schema:_interaction_scalar(fields['kind], 'kind, path + ".kind", wire);
    const result = $document_schema:_interaction_record(value, tosym(kind), path, wire);
    $document_schema:_utf16_length(generate_json(result)) <= 2048
      || raise(E_INVARG, path + ": descriptor exceeds 2048 UTF-16 units");
    return result;
  endmethod

  method _interaction_shape owner: HACKER
    "Define descriptor records using the web interaction protocol's fields and bounds.";
    const {kind} = args;
    const shapes = [
      'object -> {['kind -> 'kind, 'ref -> 'ref], ['object_kind -> 'object_kind]},
      'help -> {['kind -> 'kind, 'provider -> 'ref, 'topic -> 256], []},
      'verb -> {['kind -> 'kind, 'receiver -> 'ref, 'name -> 256], ['definer -> 'ref]},
      'property -> {['kind -> 'kind, 'object -> 'ref, 'name -> 256], []},
      'change -> {['kind -> 'kind, 'provider -> 'ref, 'review -> 'positive,
        'generation -> 'positive], ['row -> 256]},
      'command -> {['kind -> 'kind], ['command -> 1024, 'template -> 1024,
        'arguments -> 'arguments, 'action -> 'action, 'exit -> 'exit]},
      'action -> {['label -> 80], ['icon -> 32, 'title -> 256]},
      'exit -> {['source -> 'ref, 'destination -> 'ref, 'passage -> 128], []},
      'field -> {['label -> 256, 'expected_kind -> 'expected_kind],
        ['required -> 'boolean, 'suggestions -> 'suggestions, 'bound -> 'bound]},
      'suggestions -> {['provider -> 'ref, 'source -> 256], []},
      'bound -> {['value -> 256, 'label -> 256], []}
    ];
    return shapes[kind];
  endmethod

  method _interaction_fields owner: HACKER
    "Decode exact descriptor keys. Internal descriptors use symbols and snake_case names.";
    const {value, path, wire} = args;
    typeof(value) == TYPE_MAP || raise(E_INVARG, path + ": expected descriptor record");
    length(value) <= 16 || raise(E_INVARG, path + ": too many descriptor fields");
    let fields = [];
    for key in (mapkeys(value))
      typeof(key) == (wire ? TYPE_STR | TYPE_SYM)
        || raise(E_INVARG, path + ": invalid descriptor field type");
      let name = wire ? tosym(key) | key;
      if (wire)
        if (strcmp(key, "objectKind") == 0)
          name = 'object_kind;
        elseif (strcmp(key, "expectedKind") == 0)
          name = 'expected_kind;
        else
          match(key, "^[a-z_]+$", 1) || raise(E_INVARG, path + ": invalid descriptor field spelling");
          !(name in {'object_kind, 'expected_kind})
            || raise(E_INVARG, path + ": descriptor metadata uses camelCase for this field");
        endif
      else
        match(tostr(name), "^[a-z_]+$", 1)
          || raise(E_INVARG, path + ": invalid descriptor field spelling");
      endif
      !maphaskey(fields, name) || raise(E_INVARG, path + ": duplicate descriptor field");
      fields[name] = value[key];
    endfor
    return fields;
  endmethod

  method _interaction_record owner: HACKER
    "Validate an interaction record recursively through its fixed schema, never arbitrary values.";
    const {value, kind, path, wire} = args;
    const fields = $document_schema:_interaction_fields(value, path, wire);
    if (kind == 'arguments)
      length(fields) in {1, 2} || raise(E_INVARG, path + ": expected one or two arguments");
      let arguments = [];
      for key in (mapkeys(fields))
        key in {'dobj, 'iobj} || raise(E_INVARG, path + ": unknown argument slot");
        arguments[tostr(key)] = $document_schema:_interaction_record(
          fields[key], 'field, path + "." + tostr(key), wire);
      endfor
      return arguments;
    endif
    const {required, optional} = $document_schema:_interaction_shape(kind);
    for key in (mapkeys(fields))
      (maphaskey(required, key) || maphaskey(optional, key))
        || raise(E_INVARG, path + ": unknown descriptor field " + tostr(key));
    endfor
    let result = [];
    for key in ({@mapkeys(required), @mapkeys(optional)})
      if (!maphaskey(fields, key))
        !maphaskey(required, key) || raise(E_INVARG, path + "." + tostr(key) + ": missing field");
        continue;
      endif
      const rule = maphaskey(required, key) ? required[key] | optional[key];
      const field_path = path + "." + tostr(key);
      let encoded = false;
      if (rule in {'arguments, 'field, 'action, 'exit, 'suggestions, 'bound})
        encoded = $document_schema:_interaction_record(fields[key], rule, field_path, wire);
      else
        encoded = $document_schema:_interaction_scalar(fields[key], rule, field_path, wire);
      endif
      let name = tostr(key);
      key == 'object_kind && (name = "objectKind");
      key == 'expected_kind && (name = "expectedKind");
      result[name] = encoded;
    endfor
    if (kind == 'command)
      if (maphaskey(result, "command"))
        (!maphaskey(result, "template") && !maphaskey(result, "arguments"))
          || raise(E_INVARG, path + ": command and template are mutually exclusive");
      else
        (maphaskey(result, "template") && maphaskey(result, "arguments")
          && !maphaskey(result, "exit")) || raise(E_INVARG, path + ": incomplete command template");
        let rest = result["template"];
        for slot in (mapkeys(result["arguments"]))
          const marker = "{" + slot + "}";
          const position = index(rest, marker, 1);
          position > 0 || raise(E_INVARG, path + ": argument missing from template");
          rest = rest[1..position - 1] + rest[position + length(marker)..$];
        endfor
        (!index(rest, "{") && !index(rest, "}"))
          || raise(E_INVARG, path + ": unknown or repeated template placeholder");
      endif
    elseif (kind == 'field && result["expectedKind"] == "object" && maphaskey(result, "bound"))
      $document_schema:_interaction_scalar(result["bound"]["value"], 'ref, path + ".bound.value", true);
    endif
    return result;
  endmethod

  method _interaction_scalar owner: HACKER
    "Validate descriptor scalars, exact enum spellings, opaque references, and text bounds.";
    const {value, rule, path, wire} = args;
    if (rule in {'positive, 'boolean})
      return $document_schema:_scalar(value, rule, path, wire);
    elseif (rule in {'kind, 'object_kind, 'expected_kind})
      typeof(value) == (wire ? TYPE_STR | TYPE_SYM)
        || raise(E_INVARG, path + ": invalid descriptor enum type");
      const enums = [
        'kind -> {'object, 'command, 'help, 'verb, 'property, 'change},
        'object_kind -> {'object, 'person, 'container}, 'expected_kind -> {'object, 'text}
      ];
      for allowed in (enums[rule])
        if (strcmp(tostr(value), tostr(allowed)) == 0)
          return tostr(allowed);
        endif
      endfor
      raise(E_INVARG, path + ": unknown descriptor enum");
    endif
    typeof(value) == TYPE_STR || raise(E_INVARG, path + ": expected descriptor text");
    const limit = rule == 'ref ? 64 | rule;
    (length(value) > 0 && length(value) <= limit && $document_schema:_utf16_length(value) <= limit)
      || raise(E_INVARG, path + ": invalid descriptor text length");
    const bytes = binary_from_str(value);
    for i in [1..length(bytes)]
      (bytes[i] >= 32 && bytes[i] != 127)
        || raise(E_INVARG, path + ": descriptor text contains a control character");
    endfor
    if (rule == 'ref)
      let valid_ref = !!match(value, "^oid:[0-9]+$", 1);
      if (!valid_ref && length(value) == 22 && strcmp(value[1..5], "uuid:") == 0)
        valid_ref = value[12] == "-" && !!match(value[6..11] + value[13..22], "^[0-9a-fA-F]+$", 1);
      endif
      valid_ref || raise(E_INVARG, path + ": invalid object reference");
    endif
    return value;
  endmethod

  method _utf16_length owner: HACKER
    "Count UTF-16 units to match the web descriptor protocol, including non-BMP characters.";
    const {text} = args;
    const bytes = binary_from_str(text);
    let count = 0;
    for i in [1..length(bytes)]
      const byte = bytes[i];
      if (byte < 128 || byte >= 192)
        count = count + (byte >= 240 ? 2 | 1);
      endif
    endfor
    return count;
  endmethod

  method _descriptor_from_wire owner: HACKER
    "Restore symbol keys and enum values in an already validated interaction descriptor.";
    const {encoded} = args;
    let result = [];
    for name in (mapkeys(encoded))
      let key = tosym(name);
      strcmp(name, "objectKind") == 0 && (key = 'object_kind);
      strcmp(name, "expectedKind") == 0 && (key = 'expected_kind);
      const value = encoded[name];
      if (typeof(value) == TYPE_MAP)
        result[key] = $document_schema:_descriptor_from_wire(value);
      elseif (key in {'kind, 'object_kind, 'expected_kind})
        result[key] = tosym(value);
      else
        result[key] = value;
      endif
    endfor
    return result;
  endmethod
endobject
