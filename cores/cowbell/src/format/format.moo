object FORMAT [
  import_export_id -> "format",
  import_export_hierarchy -> {"format"}
]
  name: "Format Objects"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  property annotation (owner: HACKER, flags: "r") = FORMAT_ANNOTATION;
  property block (owner: HACKER, flags: "r") = FORMAT_BLOCK;
  property code (owner: HACKER, flags: "r") = FORMAT_CODE;
  property deflist (owner: HACKER, flags: "r") = FORMAT_DEFLIST;
  property djot (owner: HACKER, flags: "r") = FORMAT_DJOT;
  property link (owner: HACKER, flags: "r") = FORMAT_LINK;
  property list (owner: HACKER, flags: "r") = FORMAT_LIST;
  property paragraph (owner: ARCH_WIZARD, flags: "r") = FORMAT_PARAGRAPH;
  property table (owner: HACKER, flags: "r") = FORMAT_TABLE;
  property text (owner: HACKER, flags: "r") = FORMAT_TEXT;
  property title (owner: HACKER, flags: "r") = FORMAT_TITLE;

  override description (owner: HACKER, flags: "rc") = "Container for formatting objects like block, list, table, and title.";

  method connection_format owner: HACKER
    "Decode a connection's advertised MIME types and select a supported core format.";
    const {offered} = args;
    typeof(offered) == TYPE_LIST || raise(E_TYPE);
    !length(offered) && return 'text_plain;
    const types = ["text/html" -> 'text_html, "text/djot" -> 'text_djot,
      "text/plain" -> 'text_plain, "text_html" -> 'text_html,
      "text_djot" -> 'text_djot, "text_plain" -> 'text_plain];
    for format in ({'text_html, 'text_djot, 'text_plain})
      for spelling in (offered)
        const name = tostr(spelling);
        if (maphaskey(types, name) && types[name] == format)
          return format;
        endif
      endfor
    endfor
    raise(E_INVARG, "Connection has no supported narrative content type");
  endmethod

  method authored owner: HACKER
    "Convert an explicitly selected source kind to inline or block content. Never infer markup.";
    const {kind, source} = args;
    if (kind == 'literal)
      return $document_template:mk('blocks, $format:literal_blocks(source));
    elseif (kind == 'djot_blocks)
      return $document_template:mk('blocks, $format.djot:blocks(source));
    elseif (kind == 'djot_inline)
      return $document_template:mk('inline, $format.djot:inline(source));
    endif
    raise(E_INVARG, "Content kind must be literal, djot_blocks, or djot_inline");
  endmethod

  method literal_inline owner: HACKER
    "Convert literal text to inline nodes. Newlines become hard breaks; punctuation stays literal.";
    const {text} = args;
    typeof(text) == TYPE_STR || raise(E_TYPE, "Literal text must be a string");
    length(binary_from_str(text)) <= 262144 || raise(E_QUOTA, "Literal text exceeds 256 KiB");
    const normalized = strsub(strsub(text, "\r\n", "\n", 1), "\r", "\n", 1);
    const lines = $str_proto:split(normalized, "\n");
    let nodes = {};
    for i in [1..length(lines)]
      i > 1 && (nodes = {@nodes, $document_node:mk('hard_break)});
      length(lines[i]) && (nodes = {@nodes, $document_node:mk('text, {}, ['text -> lines[i]])});
    endfor
    return nodes;
  endmethod

  method literal_blocks owner: HACKER
    "Convert literal prose to paragraphs separated by blank lines, retaining other line breaks.";
    const {text} = args;
    typeof(text) == TYPE_STR || raise(E_TYPE, "Literal prose must be a string");
    length(binary_from_str(text)) <= 262144 || raise(E_QUOTA, "Literal prose exceeds 256 KiB");
    const normalized = strsub(strsub(text, "\r\n", "\n", 1), "\r", "\n", 1);
    let blocks = {};
    let lines = {};
    for line in ({@$str_proto:split(normalized, "\n"), ""})
      if (length(line:trim()))
        lines = {@lines, line};
      elseif (length(lines))
        blocks = {@blocks, $document_node:mk('paragraph,
          $format:literal_inline($list_proto:join(lines, "\n")))};
        lines = {};
      endif
    endfor
    return blocks;
  endmethod

  method inlines owner: HACKER
    "Normalize authored inline parts. Strings are literal, and lists are explicit inline sequences.";
    const {parts, ?depth = 0} = args;
    typeof(parts) == TYPE_LIST || raise(E_TYPE, "Inline parts must be a list");
    depth <= 32 || raise(E_QUOTA, "Inline authoring nesting exceeds 32 levels");
    let nodes = {};
    for part in (parts)
      if (typeof(part) == TYPE_STR)
        nodes = {@nodes, @$format:literal_inline(part)};
      elseif (typeof(part) == TYPE_LIST)
        nodes = {@nodes, @$format:inlines(part, depth + 1)};
      else
        $format:role(part) == 'inline || raise(E_INVARG, "Expected inline content");
        nodes = {@nodes, part};
      endif
      length(nodes) <= 8192 || raise(E_QUOTA, "Too many inline authoring nodes");
    endfor
    return nodes;
  endmethod

  method blocks owner: HACKER
    "Group adjacent inline parts into paragraphs and retain explicit block boundaries.";
    const {parts} = args;
    typeof(parts) == TYPE_LIST || raise(E_TYPE, "Block parts must be a list");
    let blocks = {};
    let inlines = {};
    for part in (parts)
      if (typeof(part) == TYPE_FLYWEIGHT && $format:role(part) == 'block)
        if (length(inlines))
          blocks = {@blocks, $document_template:mk('paragraph, inlines)};
          inlines = {};
        endif
        blocks = {@blocks, part};
      else
        inlines = {@inlines, @$format:inlines({part})};
      endif
      length(blocks) + length(inlines) <= 8192 || raise(E_QUOTA, "Too many authoring nodes");
    endfor
    if (length(inlines))
      blocks = {@blocks, $document_template:mk('paragraph, inlines)};
    endif
    return blocks;
  endmethod

  method role owner: HACKER
    "Classify only known node and template delegates, without calling content methods.";
    const {value} = args;
    typeof(value) == TYPE_FLYWEIGHT || raise(E_TYPE, "Expected content flyweight");
    value.delegate == $sub && return 'inline;
    value.delegate in {$document_node, $document_template}
      || raise(E_INVARG, "Custom content needs an explicit producer constructor");
    const fields = flyslots(value);
    maphaskey(fields, 'kind) || raise(E_INVARG, "Content kind is missing");
    const kind = fields['kind];
    if (value.delegate == $document_template)
      kind in {'inline, 'capitalize, 'producer_inline} && return 'inline;
      kind in {'blocks, 'producer_blocks} && return 'block;
    endif
    const role = $document_schema:_shape(kind, "template")['group];
    role in {'inline, 'block} || raise(E_INVARG, "Expected an inline or block node");
    return role;
  endmethod

  method producer_inline owner: HACKER
    "Defer an explicit resolve_inline(context) hook until document resolution.";
    const {producer} = args;
    typeof(producer) in {TYPE_OBJ, TYPE_FLYWEIGHT} || raise(E_TYPE);
    return $document_template:mk('producer_inline, {}, ['producer -> producer]);
  endmethod

  method producer_blocks owner: HACKER
    "Defer an explicit resolve_blocks(context) hook until document resolution.";
    const {producer} = args;
    typeof(producer) in {TYPE_OBJ, TYPE_FLYWEIGHT} || raise(E_TYPE);
    return $document_template:mk('producer_blocks, {}, ['producer -> producer]);
  endmethod

endobject
