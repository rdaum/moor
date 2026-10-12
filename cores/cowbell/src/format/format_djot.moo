object FORMAT_DJOT [
  import_export_id -> "format_djot",
  import_export_hierarchy -> {"format"}
]
  name: "Djot Authoring Parser"
  parent: ROOT
  owner: HACKER
  readable: true

  method blocks owner: HACKER
    "Parse explicit Djot source into canonical blocks. Errors identify the source line and column.";
    const {source} = args;
    const lines = $format.djot:_source(source);
    "The first block walk collects definitions, including forward and nested definitions.";
    const {ignored, references} = $format.djot:_blocks(lines, [], 0, true);
    const {blocks, finished} = $format.djot:_blocks(lines, references, 0, false);
    $document:validate($document:mk(blocks));
    return blocks;
  endmethod

  method inline owner: HACKER
    "Parse one inline sequence. Paragraph boundaries and block markers are invalid here.";
    const {source} = args;
    const lines = $format.djot:_source(source);
    for line in (lines)
      if (length(lines) > 1 && !length(line[1]:trim()))
        $format.djot:_error(line, 1, "inline input cannot contain a paragraph boundary");
      endif
      $format.djot:_marker(line)[1] == 'paragraph
        || $format.djot:_error(line, 1, "inline input cannot contain a block construct");
    endfor
    const nodes = $format.djot:_inline(lines, [], 1);
    $document:validate($document:mk({$document_node:mk('paragraph, nodes)}));
    return nodes;
  endmethod

  method _source owner: HACKER
    "Normalize line endings and retain one-based source positions as {text, line, column}.";
    const {source} = args;
    typeof(source) == TYPE_STR || raise(E_TYPE, "Djot source must be a string");
    length(binary_from_str(source)) <= 65536 || raise(E_QUOTA, "Djot source exceeds 64 KiB");
    const normalized = strsub(strsub(source, "\r\n", "\n", 1), "\r", "\n", 1);
    let lines = {};
    for text in (normalized:split("\n"))
      lines = {@lines, {text, length(lines) + 1, 1}};
    endfor
    return lines;
  endmethod

  method _error owner: HACKER
    "Raise a source diagnostic at a character offset in a retained physical line.";
    const {line, offset, message} = args;
    raise(E_INVARG, "Djot line " + tostr(line[2]) + ", column "
      + tostr(line[3] + offset - 1) + ": " + message);
  endmethod

  method _at owner: HACKER
    "Locate an offset in an inline sequence without losing container indentation.";
    let {lines, offset} = args;
    for line in (lines)
      offset <= length(line[1]) + 1 && return {line, offset};
      offset = offset - length(line[1]) - 1;
    endfor
    return {lines[$], length(lines[$][1]) + 1};
  endmethod

  method _strip owner: HACKER
    "Remove a structural prefix while retaining the original source coordinates.";
    const {line, count} = args;
    return {line[1][count + 1..$], line[2], line[3] + count};
  endmethod

  method _match owner: HACKER
    "Match one PCRE pattern and retain character positions for the parser's source slices.";
    const {source, pattern, case_matters} = args;
    const found = pcre_match(source, pattern, case_matters, false);
    !length(found) && return {};
    const groups = found[1];
    const positions = {groups[tostr(i)]["position"] for i in [1..length(groups) - 1]};
    return {@groups["0"]["position"], positions};
  endmethod

  method _capture owner: HACKER
    "Extract one regular-expression capture.";
    const {source, matched, group} = args;
    const range = matched[3][group];
    return source[range[1]..range[2]];
  endmethod

  method _text owner: HACKER
    "Make an inert text node.";
    const {text} = args;
    return $document_node:mk('text, {}, ['text -> text]);
  endmethod

  method _nodes owner: HACKER
    "Turn unmatched delimiter tokens into literal text and coalesce adjacent text nodes.";
    const {tokens} = args;
    let nodes = {};
    for token in (tokens)
      const node = typeof(token) == TYPE_MAP ? $format.djot:_text(token['raw]) | token;
      if (node.kind == 'text && length(nodes) && nodes[$].kind == 'text)
        nodes[$] = $format.djot:_text(nodes[$].text + node.text);
      else
        nodes = {@nodes, node};
      endif
    endfor
    return nodes;
  endmethod

  method _plain owner: HACKER
    "Read an image alternative or implicit reference label from already parsed inline nodes.";
    let {nodes} = args;
    let parts = {};
    let count = 0;
    while (length(nodes))
      count = count + 1;
      count <= 8192 || raise(E_QUOTA, "Djot label exceeds 8192 nodes");
      const node = nodes[1];
      nodes = nodes[2..$];
      if (node.kind in {'text, 'inline_code})
        parts = {@parts, node.text};
      elseif (node.kind in {'soft_break, 'hard_break})
        parts = {@parts, " "};
      elseif (node.kind == 'image)
        parts = {@parts, node.alt};
      else
        nodes = {@flycontents(node), @nodes};
      endif
    endwhile
    return parts:join("");
  endmethod

  method _nesting owner: HACKER
    "Bound inline containers before a link or image can consume their parsed labels.";
    const {nodes, depth} = args;
    let pending = {{node, depth} for node in (nodes)};
    while (length(pending))
      const {node, level} = pending[1];
      pending = pending[2..$];
      level <= 32 || raise(E_QUOTA, "Djot nesting exceeds 32 levels");
      for child in (flycontents(node))
        pending = {@pending, {child, level + 1}};
      endfor
    endwhile
  endmethod

  method _url owner: HACKER
    "Apply the document URL policy at the authored destination's source location.";
    const {url, image, lines, offset} = args;
    const {line, column} = $format.djot:_at(lines, offset);
    try
      return $document_schema:_scalar(url, image ? 'image_url | 'url, "destination", false);
    except error (E_INVARG)
      $format.djot:_error(line, column, error[2]);
    endtry
  endmethod

  method _marker owner: HACKER
    "Classify a block start as {kind, prefix length, data}. Indentation is a container concern.";
    const {line} = args;
    const source = line[1];
    const indent = $format.djot:_match(source, "^[ \t]*", 1)[2];
    const text = source[indent + 1..$];
    !length(text) && return {'blank, indent, false};
    const fence = $format.djot:_match(text, "^(`{3,})[ \t]*(.*)$", 1);
    if (fence)
      return {'code_block, indent, {$format.djot:_capture(text, fence, 1),
        $format.djot:_capture(text, fence, 2):trim()}};
    endif
    const heading = $format.djot:_match(text, "^(#{1,})[ \t]+", 1);
    if (heading)
      return {'heading, indent + heading[2], length($format.djot:_capture(text, heading, 1))};
    endif
    const compact = strsub(strsub(text, " ", "", 1), "\t", "", 1);
    $format.djot:_match(compact, "^(---+|\\*\\*\\*+)$", 1) && return {'thematic_break, indent, false};
    if (text == ">" || text:starts_with("> ") || text:starts_with(">\t"))
      return {'block_quote, indent + min(length(text), 2), false};
    endif
    if (text:starts_with(":::") || text:starts_with("[^"))
      return {'unsupported, indent, "divs and footnotes are not supported"};
    endif
    const definition = $format.djot:_match(text, "^\\[([^]]+)\\]:([ \t]+|$)", 1);
    if (definition)
      return {'link_definition, indent + definition[2], $format.djot:_capture(text, definition, 1)};
    endif
    const list = $format.djot:_match(text, "^([-+*:]|[0-9]+[.)]|\\([0-9]+\\))([ \t]+|$)", 1);
    if (list)
      const marker = $format.djot:_capture(text, list, 1);
      let style = marker;
      let start = 1;
      if (length(marker) > 1)
        try
          start = toint(strsub(strsub(strsub(marker, "(", "", 1), ")", "", 1), ".", "", 1));
          $document_schema:_scalar(start, 'integer, "ordered list start", false);
        except (E_INVARG, E_RANGE)
          $format.djot:_error(line, indent + 1, "ordered list start is outside the supported integer range");
        endtry
        style = marker:starts_with("(") ? "(n)" | marker:ends_with(")") ? "n)" | "n.";
      endif
      return {'list, indent + list[2], ['marker -> style, 'start -> start, 'indent -> indent]};
    endif
    if ($format.djot:_match(text, "^(([A-Za-z]|[ivxlcdmIVXLCDM]+)[.)]|\\(([A-Za-z]|[ivxlcdmIVXLCDM]+)\\))[ \t]+", 1))
      return {'unsupported, indent, "only decimal ordered lists are supported"};
    endif
    text:starts_with("|") && text:ends_with("|") && return {'table, indent, false};
    text:starts_with("^ ") && return {'caption, indent + 2, false};
    return {'paragraph, indent, false};
  endmethod

  method _blocks owner: HACKER
    "Parse container lines directly into blocks. A scan-only pass gathers link definitions.";
    let {lines, references, depth, scan} = args;
    depth <= 32 || raise(E_QUOTA, "Djot nesting exceeds 32 levels");
    let blocks = {};
    let cursor = 1;
    while (cursor <= length(lines))
      length(blocks) <= 8192 || raise(E_QUOTA, "Djot has too many blocks");
      const line = lines[cursor];
      const {kind, prefix, data} = $format.djot:_marker(line);
      if (kind == 'blank)
        cursor = cursor + 1;
        continue;
      elseif (kind == 'unsupported)
        $format.djot:_error(line, prefix + 1, data);
      elseif (kind == 'link_definition)
        let url = line[1][prefix + 1..$]:trim();
        cursor = cursor + 1;
        while (cursor <= length(lines) && length(lines[cursor][1]:trim()))
          const continuation = lines[cursor];
          const indent = $format.djot:_match(continuation[1], "^[ \t]*", 1)[2];
          if (!indent)
            break;
          endif
          url = url + continuation[1]:trim();
          cursor = cursor + 1;
        endwhile
        if (scan)
          "Base32 preserves case-distinct labels in MOO's case-insensitive string-key maps.";
          references[encode_base32(data)] = $format.djot:_url(url, false, {line}, prefix + 1);
        endif
        continue;
      elseif (kind == 'code_block)
        const {fence, language} = data;
        language:starts_with("=") && $format.djot:_error(line, prefix + 1, "raw output is not supported");
        (!length(language) || $format.djot:_match(language, "^[A-Za-z0-9_.+-]+$", 1))
          || $format.djot:_error(line, prefix + 1, "invalid code language or attributes");
        let code = {};
        cursor = cursor + 1;
        while (cursor <= length(lines))
          const text = lines[cursor][1];
          if ($format.djot:_match(text:trim(), "^`+$", 1) && length(text:trim()) >= length(fence))
            cursor = cursor + 1;
            break;
          endif
          code = {@code, text};
          cursor = cursor + 1;
        endwhile
        if (!scan)
          blocks = {@blocks, $document_node:mk('code_block, {},
            ['text -> length(code) ? code:join("\n") + "\n" | "", 'language -> language])};
        endif
        continue;
      elseif (kind == 'thematic_break)
        !scan && (blocks = {@blocks, $document_node:mk('thematic_break)});
        cursor = cursor + 1;
        continue;
      elseif (kind == 'block_quote)
        let quoted = {};
        let paragraph = false;
        while (cursor <= length(lines))
          const current = lines[cursor];
          const marker = $format.djot:_marker(current);
          if (marker[1] == 'block_quote)
            const inner = $format.djot:_strip(current, marker[2]);
            quoted = {@quoted, inner};
            paragraph = $format.djot:_marker(inner)[1] == 'paragraph && length(inner[1]:trim());
          elseif (paragraph && length(current[1]:trim()))
            quoted = {@quoted, current};
          else
            break;
          endif
          cursor = cursor + 1;
        endwhile
        const {children, updated} = $format.djot:_blocks(quoted, references, depth + 1, scan);
        references = updated;
        !scan && (blocks = {@blocks, $document_node:mk('block_quote, children)});
        continue;
      elseif (kind == 'list)
        const {list, after, updated} = $format.djot:_list(lines, cursor, references, depth, scan);
        references = updated;
        cursor = after;
        !scan && (blocks = {@blocks, list});
        continue;
      elseif (kind == 'table)
        const {tables, after} = $format.djot:_table(lines, cursor, references, depth, scan);
        blocks = {@blocks, @tables};
        cursor = after;
        continue;
      elseif (kind == 'caption)
        $format.djot:_error(line, prefix + 1, "a caption must follow a table");
      endif
      "Paragraphs and headings continue to the next blank line; block markers cannot interrupt them.";
      kind != 'heading || data <= 6
        || $format.djot:_error(line, 1, "heading levels must be 1 through 6");
      let paragraph = {$format.djot:_strip(line, prefix)};
      cursor = cursor + 1;
      while (cursor <= length(lines) && length(lines[cursor][1]:trim()))
        const current = lines[cursor];
        const marker = $format.djot:_marker(current);
        const same_heading = kind == 'heading && marker[1] == 'heading && marker[3] == data;
        const count = same_heading ? marker[2] | $format.djot:_match(current[1], "^[ \t]*", 1)[2];
        paragraph = {@paragraph, $format.djot:_strip(current, count)};
        cursor = cursor + 1;
      endwhile
      if (!scan)
        const contents = $format.djot:_inline(paragraph, references, depth + 1);
        blocks = {@blocks, $document_node:mk(kind, contents, kind == 'heading ? ['level -> data] | [])};
      endif
    endwhile
    return {blocks, references};
  endmethod

  method _list owner: HACKER
    "Collect same-style items, preserving lazy paragraphs and indented nested block content.";
    let {lines, cursor, references, depth, scan} = args;
    const first = $format.djot:_marker(lines[cursor])[3];
    const marker = first['marker];
    let items = {};
    while (cursor <= length(lines))
      const entry = $format.djot:_marker(lines[cursor]);
      if (entry[1] != 'list || entry[3]['marker] != marker || entry[3]['indent] != first['indent])
        break;
      endif
      let initial = $format.djot:_strip(lines[cursor], entry[2]);
      let checked = false;
      let task = false;
      const check = $format.djot:_match(initial[1], "^\\[([ xX])\\][ \t]+", 1);
      if (marker in {"-", "+", "*"} && check)
        task = true;
        checked = $format.djot:_capture(initial[1], check, 1) != " ";
        initial = $format.djot:_strip(initial, check[2]);
      endif
      let inner = {initial};
      let paragraph = $format.djot:_marker(initial)[1] == 'paragraph;
      cursor = cursor + 1;
      while (cursor <= length(lines))
        const line = lines[cursor];
        const indent = $format.djot:_match(line[1], "^[ \t]*", 1)[2];
        if (!length(line[1]:trim()))
          inner = {@inner, line};
          paragraph = false;
        elseif (indent > first['indent])
          const stripped = $format.djot:_strip(line, min(indent, entry[2]));
          if (!paragraph)
            paragraph = $format.djot:_marker(stripped)[1] == 'paragraph;
          endif
          inner = {@inner, stripped};
        elseif ($format.djot:_marker(line)[1] == 'list || !paragraph)
          break;
        else
          inner = {@inner, line};
        endif
        cursor = cursor + 1;
      endwhile
      const {children, updated} = $format.djot:_blocks(inner, references, depth + 2, scan);
      references = updated;
      if (!scan)
        if (marker == ":")
          (length(children) && children[1].kind == 'paragraph)
            || $format.djot:_error(initial, 1, "a definition item needs a term");
          items = {@items, ['term -> flycontents(children[1]), 'definition -> children[2..$]]};
        else
          let item = ['blocks -> children];
          task && (item['checked] = checked);
          items = {@items, item};
        endif
      endif
    endwhile
    const fields = marker in {"-", "+", "*"}
      ? ['style -> 'bullet] | ['style -> 'numbered, 'start -> first['start]];
    const list = marker == ":" ? $document_node:mk('definition_list, items)
      | $document_node:mk('list, items, fields);
    return {list, cursor, references};
  endmethod

  method _cells owner: HACKER
    "Split a pipe row, excluding escaped pipes and pipes inside exact-width verbatim spans.";
    const {line} = args;
    const text = line[1];
    let cells = {};
    let start = index(text, "|") + 1;
    let cursor = start;
    let code = 0;
    while (cursor <= length(text))
      if (text[cursor] == "\\" && !code)
        cursor = cursor + 2;
        continue;
      elseif (text[cursor] == "`")
        const width = $format.djot:_match(text[cursor..$], "^`+", 1)[2];
        code = code == width ? 0 | !code ? width | code;
        cursor = cursor + width;
        continue;
      elseif (text[cursor] == "|" && !code)
        const content = text[start..cursor - 1];
        const indent = $format.djot:_match(content, "^[ \t]*", 1)[2];
        cells = {@cells, {content:trim(), line[2], line[3] + start + indent - 1}};
        start = cursor + 1;
      endif
      cursor = cursor + 1;
    endwhile
    start > length(text:trimr())
      || $format.djot:_error(line, start, "table row needs a closing pipe outside code");
    return cells;
  endmethod

  method _table owner: HACKER
    "Parse pipe cells and captions. A new alignment/header segment becomes a separate table.";
    let {lines, cursor, references, depth, scan} = args;
    let tables = {};
    let rows = {};
    let fields = ['caption -> {}, 'columns -> {}];
    let width = 0;
    while (cursor <= length(lines) && $format.djot:_marker(lines[cursor])[1] == 'table)
      const line = lines[cursor];
      const cells = $format.djot:_cells(line);
      !width && (width = length(cells));
      length(cells) == width || $format.djot:_error(line, 1, "inconsistent table column count");
      let separator = true;
      for cell in (cells)
        !$format.djot:_match(cell[1], "^:?-+:?$", 1) && (separator = false);
      endfor
      if (separator)
        if (length(rows) > 1 || maphaskey(fields, 'header))
          if (!scan)
            if (!length(fields['columns]))
              fields['columns] = {['align -> 'start] for i in [1..width]};
            endif
            tables = {@tables, $document_node:mk('table, rows[1..$ - 1], fields)};
          endif
          fields = ['caption -> {}, 'columns -> {}];
        endif
        if (length(rows))
          const last_row = rows[length(rows)];
          fields['header] = {flycontents(cell['blocks][1]) for cell in (last_row['cells])};
        endif
        rows = {};
        fields['columns] = {['align -> cell[1]:ends_with(":")
          ? (cell[1]:starts_with(":") ? 'center | 'end) | 'start] for cell in (cells)};
      elseif (!scan)
        let values = {};
        for cell in (cells)
          const nodes = $format.djot:_inline({cell}, references, depth + 4);
          values = {@values, ['blocks -> {$document_node:mk('paragraph, nodes)}]};
        endfor
        rows = {@rows, ['cells -> values]};
      endif
      cursor = cursor + 1;
    endwhile
    if (!length(fields['columns]))
      fields['columns] = {['align -> 'start] for i in [1..width]};
    endif
    let next = cursor;
    while (next <= length(lines) && !length(lines[next][1]:trim()))
      next = next + 1;
    endwhile
    if (next <= length(lines) && $format.djot:_marker(lines[next])[1] == 'caption)
      const prefix = $format.djot:_marker(lines[next])[2];
      let caption = {$format.djot:_strip(lines[next], prefix)};
      cursor = next + 1;
      while (cursor <= length(lines) && length(lines[cursor][1]:trim()))
        const indent = $format.djot:_match(lines[cursor][1], "^[ \t]*", 1)[2];
        if (!indent)
          break;
        endif
        caption = {@caption, $format.djot:_strip(lines[cursor], indent)};
        cursor = cursor + 1;
      endwhile
      !scan && (fields['caption] = $format.djot:_inline(caption, references, depth + 1));
    endif
    !scan && (tables = {@tables, $document_node:mk('table, rows, fields)});
    return {tables, cursor};
  endmethod

  method _inline owner: HACKER
    "Scan inline delimiters left to right. The first matched closer fixes container precedence.";
    const {lines, references, depth} = args;
    depth <= 32 || raise(E_QUOTA, "Djot nesting exceeds 32 levels");
    const source = {line[1] for line in (lines)}:join("\n");
    let tokens = {};
    let cursor = 1;
    while (cursor <= length(source))
      length(tokens) <= 8192 || raise(E_QUOTA, "Djot has too many inline nodes");
      const tail = source[cursor..$];
      const char = source[cursor];
      const next = cursor < length(source) ? source[cursor + 1] | "";
      const previous = cursor > 1 ? source[cursor - 1] | " ";
      const {line, column} = $format.djot:_at(lines, cursor);
      "Consume ordinary text in runs, so prose does not require a node per character.";
      const ordinary = $format.djot:_match(tail, "^[^\\\\`*_{}\\[\\]<>!~^$\"'\\n.\\-]+", 1);
      if (ordinary)
        tokens = {@tokens, $format.djot:_text(tail[1..ordinary[2]])};
        cursor = cursor + ordinary[2];
        continue;
      endif
      if (char == "\\")
        const hard = $format.djot:_match(tail, "^\\\\[ \t]*\n", 1);
        if (hard)
          if (length(tokens) && typeof(tokens[$]) == TYPE_FLYWEIGHT && tokens[$].kind == 'text)
            tokens[$] = $format.djot:_text(tokens[$].text:trimr());
          endif
          tokens = {@tokens, $document_node:mk('hard_break)};
          cursor = cursor + hard[2];
          continue;
        elseif (next == " ")
          tokens = {@tokens, $format.djot:_text(" ")};
          cursor = cursor + 2;
          continue;
        elseif (length(binary_from_str(next)) == 1 && $format.djot:_match(next, "^[[:punct:]]$", 1))
          tokens = {@tokens, $format.djot:_text(next)};
          cursor = cursor + 2;
          continue;
        endif
      elseif (char == "\n")
        tokens = {@tokens, $document_node:mk('soft_break)};
        cursor = cursor + 1;
        continue;
      elseif (char == "`")
        const width = $format.djot:_match(tail, "^`+", 1)[2];
        let closing = cursor + width;
        let found = false;
        while (closing <= length(source))
          const offset = index(source[closing..$], "`");
          if (!offset)
            break;
          endif
          closing = closing + offset - 1;
          const run = $format.djot:_match(source[closing..$], "^`+", 1)[2];
          if (run == width)
            found = true;
            break;
          endif
          closing = closing + run;
        endwhile
        !found && (closing = length(source) + 1);
        let text = source[cursor + width..closing - 1];
        if (text:starts_with(" `"))
          text = text[2..$];
        endif
        if (text:ends_with("` "))
          text = text[1..$ - 1];
        endif
        tokens = {@tokens, $document_node:mk('inline_code, {}, ['text -> text])};
        cursor = found ? closing + width | closing;
        continue;
      elseif ((char == "$" && (next == "`" || tail:starts_with("$$`")))
          || tail:starts_with("[^"))
        $format.djot:_error(line, column, "math and footnotes are not supported");
      elseif (char == "{")
        if ($format.djot:_match(tail, "(?s)^\\{[=+~^].*?[=+~^]\\}", 1)
            || $format.djot:_match(tail, "^\\{=[A-Za-z][A-Za-z0-9_-]*\\}", 1)
            || $format.djot:_match(tail, "^\\{[.#%][^}]*\\}", 1)
            || $format.djot:_match(tail, "^\\{[A-Za-z_][^}]*=.+\\}", 1))
          $format.djot:_error(line, column, "this Djot extension or attribute is not supported");
        endif
      elseif (char in {"~", "^"})
        const offset = index(source[cursor + 1..$], char);
        if (offset > 1 && !index(source[cursor + 1..cursor + offset - 1], "\n"))
          $format.djot:_error(line, column, "subscript and superscript are not supported");
        endif
      elseif (char == "<")
        const end = index(tail, ">");
        if (end > 1)
          const address = tail[2..end - 1];
          if (!index(address, "\n") && ($format.djot:_match(address, "^[A-Za-z][A-Za-z0-9+.-]*:", 1)
              || $format.djot:_match(address, "^[^ <>@]+@[^ <>@]+$", 1)))
            const url = index(address, "@") && !index(address, ":")
              ? "mailto:" + address | address;
            tokens = {@tokens, $document_node:mk('link, {$format.djot:_text(address)},
              ['url -> $format.djot:_url(url, false, lines, cursor)])};
            cursor = cursor + end;
            continue;
          endif
        endif
      endif
      "Bracket tokens share the delimiter stack with emphasis, preventing overlapping containers.";
      if (char == "[" || (char == "!" && next == "["))
        const image = char == "!";
        tokens = {@tokens, ['marker -> "[", 'raw -> image ? "![" | "[", 'image -> image]};
        cursor = cursor + (image ? 2 | 1);
        continue;
      elseif (char == "]" && next in {"(", "["})
        let opening = length(tokens);
        while (opening > 0)
          if (typeof(tokens[opening]) == TYPE_MAP && tokens[opening]['marker] == "[")
            break;
          endif
          opening = opening - 1;
        endwhile
        if (opening > 0)
          const label = $format.djot:_nodes(tokens[opening + 1..$]);
          let end = cursor + 2;
          let url = "";
          if (next == "[")
            const offset = index(source[end..$], "]");
            offset || $format.djot:_error(line, column, "unclosed reference label");
            const name = source[end..end + offset - 2];
            const key = length(name) ? name | $format.djot:_plain(label);
            maphaskey(references, encode_base32(key))
              || $format.djot:_error(line, column, "undefined link reference " + key);
            url = references[encode_base32(key)];
            end = end + offset - 1;
          else
            let nesting = 1;
            while (end <= length(source))
              const part = source[end];
              if (part == "\\" && end < length(source))
                end = end + 1;
                url = url + source[end];
              elseif (part == "(")
                nesting = nesting + 1;
                url = url + part;
              elseif (part == ")")
                nesting = nesting - 1;
                if (!nesting)
                  break;
                endif
                url = url + part;
              else
                url = url + part;
              endif
              end = end + 1;
            endwhile
            !nesting || $format.djot:_error(line, column, "unclosed link destination");
            url = {part:trim() for part in (url:split("\n"))}:join("");
          endif
          const image = tokens[opening]['image];
          $format.djot:_nesting(label, depth + 2);
          url = $format.djot:_url(url, image, lines, cursor);
          const node = image
            ? $document_node:mk('image, {}, ['src -> url, 'alt -> $format.djot:_plain(label)])
            | $document_node:mk('link, label, ['url -> url]);
          tokens = {@tokens[1..opening - 1], node};
          cursor = end + 1;
          continue;
        endif
      endif
      let marker = char;
      let explicit_open = false;
      let explicit_close = false;
      if (char == "{" && next in {"*", "_", "-", "\"", "'"})
        marker = next;
        explicit_open = true;
      elseif (next == "}" && char in {"*", "_", "-", "\"", "'"})
        explicit_close = true;
      endif
      if (marker in {"*", "_"} || (marker == "-" && (explicit_open || explicit_close)))
        const marked = explicit_open || explicit_close;
        let opening = length(tokens);
        const can_close = explicit_close || (!explicit_open && !index(" \t\n", previous));
        if (can_close)
          while (opening > 0)
            const token = tokens[opening];
            if (typeof(token) == TYPE_MAP && token['marker] == marker
                && token['explicit] == marked && opening < length(tokens)
                && length(strsub(source[token['offset] + 1..cursor - 1], marker, "", 1)))
              break;
            endif
            opening = opening - 1;
          endwhile
        else
          opening = 0;
        endif
        if (opening)
          const kind = marker == "*" ? 'strong | marker == "_" ? 'emphasis | 'strike;
          const contents = $format.djot:_nodes(tokens[opening + 1..$]);
          $format.djot:_nesting(contents, depth + 2);
          tokens = {@tokens[1..opening - 1], $document_node:mk(kind, contents)};
          cursor = cursor + (explicit_close ? 2 | 1);
          continue;
        elseif (explicit_open || (!explicit_close && length(next) && !index(" \t\n", next)))
          tokens = {@tokens, ['marker -> marker, 'explicit -> marked, 'offset -> cursor,
            'raw -> explicit_open ? "{" + marker | marker]};
          cursor = cursor + (explicit_open ? 2 | 1);
          continue;
        endif
      elseif (marker in {"\"", "'"})
        const marked = explicit_open || explicit_close;
        const apostrophe = marker == "'" && $format.djot:_match(previous, "[[:alnum:]]", 1)
          && $format.djot:_match(next, "[[:alnum:]]", 1);
        let opening = length(tokens);
        const can_close = explicit_close || (!explicit_open && !apostrophe
          && !index(" \t\n", previous));
        if (can_close)
          while (opening > 0)
            const token = tokens[opening];
            if (typeof(token) == TYPE_MAP && token['marker] == marker
                && token['explicit] == marked)
              break;
            endif
            opening = opening - 1;
          endwhile
        else
          opening = 0;
        endif
        const left = marker == "\"" ? "“" | "‘";
        const right = marker == "\"" ? "”" | "’";
        if (opening)
          const contents = $format.djot:_nodes(tokens[opening + 1..$]);
          tokens = {@tokens[1..opening - 1], $format.djot:_text(left),
            @contents, $format.djot:_text(right)};
        elseif (explicit_open || (!explicit_close && !apostrophe && length(next)
            && !index(" \t\n", next)))
          tokens = {@tokens, ['marker -> marker, 'explicit -> marked,
            'raw -> explicit_open ? left | marker == "'" ? right | left]};
        else
          tokens = {@tokens, $format.djot:_text(explicit_close ? right | marker == "'" ? right | left)};
        endif
        cursor = cursor + (marked ? 2 | 1);
        continue;
      elseif (tail:starts_with("..."))
        tokens = {@tokens, $format.djot:_text("…")};
        cursor = cursor + 3;
        continue;
      elseif (tail:starts_with("--"))
        const width = $format.djot:_match(tail, "^-+", 1)[2];
        const size = width % 3 == 0 ? 3 | width % 2 == 0 ? 2 | 3;
        tokens = {@tokens, $format.djot:_text(size == 3 ? "—" | "–")};
        cursor = cursor + size;
        continue;
      endif
      tokens = {@tokens, $format.djot:_text(char)};
      cursor = cursor + 1;
    endwhile
    return $format.djot:_nodes(tokens);
  endmethod
endobject
