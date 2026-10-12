object DOCUMENT_RENDERER [
  import_export_id -> "document_renderer",
  import_export_hierarchy -> {"document"}
]
  name: "Document Renderers"
  parent: ROOT
  owner: HACKER
  readable: true

  method render owner: HACKER
    "Validate a canonical document, then render its inert metadata with fixed dispatch.";
    const {document, format, ?options = []} = args;
    const settings = this:_settings(format, options);
    return this:_render($document_schema:encode(document), format, settings);
  endmethod

  method render_many owner: HACKER
    "Finalize references and encode once; equivalent requests share one serialization.";
    const {document, requests} = args;
    typeof(requests) == TYPE_LIST || raise(E_TYPE, "Rendering requests must be a list");
    length(requests) <= 1024 || raise(E_QUOTA, "Too many rendering requests");
    const checked = $document_schema:_checked(document, false, true);
    let cache = [];
    let results = {};
    for request in (requests)
      const format = request['format];
      const settings = this:_settings(format, `request['options] ! E_RANGE => []');
      const key = tostr(format, ":", settings['width], ":", settings['code_line_numbers]);
      if (!maphaskey(cache, key))
        cache[key] = this:_render(checked['encoded], format, settings);
      endif
      results = {@results, cache[key]};
    endfor
    return ['document -> checked['encoded], 'results -> results,
      'diagnostics -> checked['diagnostics]];
  endmethod

  method _settings owner: HACKER
    "Validate and normalize format options before calculating a cache key.";
    const {format, options} = args;
    (typeof(format) == TYPE_SYM && format in {'text_html, 'text_djot, 'text_plain})
      || raise(E_INVARG, "render.content_type: unsupported content type");
    typeof(options) == TYPE_MAP || raise(E_INVARG, "render.options: expected map");
    for key in (mapkeys(options))
      key in {'width, 'code_line_numbers} || raise(E_INVARG, "render.options: unknown option");
    endfor
    const width = maphaskey(options, 'width) ? options['width] | 80;
    (typeof(width) == TYPE_INT && width >= 1 && width <= 1024)
      || raise(E_INVARG, "render.options.width: expected integer from 1 through 1024");
    const numbered = maphaskey(options, 'code_line_numbers) ? options['code_line_numbers] | true;
    typeof(numbered) == TYPE_BOOL || raise(E_INVARG, "render.options.code_line_numbers: expected boolean");
    return ['width -> width, 'code_line_numbers -> numbered];
  endmethod

  method _render owner: HACKER
    "Serialize checked metadata. Only renderer entry points may bypass document validation.";
    caller == this || raise(E_PERM);
    const {checked, format, settings} = args;
    const state = ['annotations -> [], 'next_anchor -> 1];
    try
      const {body, finished} = $document_renderer:_walk(
        checked, 'document, format, settings, state, "document", true);
      return ['content_type -> format, 'body -> body, 'annotations -> finished['annotations]];
    except error (ANY)
      raise(error[1], tostr(format) + " " + error[2]);
    endtry
  endmethod

  method _walk owner: HACKER
    "Render children in reading order, carrying local anchors without shared mutable state.";
    let {node, expected, format, options, state, path, interactions} = args;
    const kind = expected in {'inline, 'block} ? tosym(node["kind"]) | expected;
    const shape = $document_schema:_shape(kind, path);
    let rendered = node;
    if (kind == 'reference && interactions && format != 'text_plain)
      const anchor = "a" + tostr(state['next_anchor]);
      state['next_anchor] = state['next_anchor] + 1;
      state['annotations][anchor] = node["descriptor"];
      rendered["anchor"] = anchor;
    endif
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      const name = tostr(key);
      if (!maphaskey(node, name))
        continue;
      endif
      let parts = {};
      for i in [1..length(node[name])]
        const value = node[name][i];
        const entry_path = path + "." + name + "[" + tostr(i - 1) + "]";
        if (cardinality == 'matrix_optional)
          let line = {};
          for j in [1..length(value)]
            const {body, next_state} = $document_renderer:_walk(value[j], child_kind,
              format, options, state, entry_path + "[" + tostr(j - 1) + "]", interactions);
            state = next_state;
            line = {@line, body};
          endfor
          parts = {@parts, line};
        else
          const {body, next_state} = $document_renderer:_walk(
            value, child_kind, format, options, state, entry_path, interactions);
          state = next_state;
          parts = {@parts, body};
        endif
      endfor
      rendered[name] = parts;
    endfor
    if (shape['group] == 'record)
      return {rendered, state};
    endif
    const body = $document_renderer:_finish(kind, node, rendered, format, options, path, interactions);
    length(binary_from_str(body)) <= 524288
      || raise(E_QUOTA, tostr(format) + " " + path + ": rendered body exceeds 512 KiB");
    return {body, state};
  endmethod

  method _finish owner: HACKER
    "Serialize one known node after its children. All text enters through format-specific escaping.";
    const {kind, source, node, format, options, path, interactions} = args;
    const html = format == 'text_html;
    const djot = format == 'text_djot;
    if (kind == 'document)
      return $document_renderer:_join(node["blocks"], html ? "\n" | "\n\n", format, path);
    elseif (kind == 'text)
      return $document_renderer:_escape(node["text"], format);
    elseif (kind == 'soft_break)
      return " ";
    elseif (kind == 'hard_break)
      return html ? "<br>\n" | (djot ? "\\\n" | "\n");
    elseif (kind == 'inline_code)
      return $document_renderer:_inline_code(node["text"], format);
    elseif (kind == 'image)
      const alt = $document_renderer:_escape(node["alt"], format);
      if (html)
        return "<img src=\"" + $document_renderer:_escape(node["src"], format)
          + "\" alt=\"" + alt + "\"" + $document_renderer:_title(node, format) + ">";
      elseif (djot)
        return "![" + alt + "](" + $document_renderer:_destination(node["src"])
          + ")" + $document_renderer:_title(node, format);
      endif
      return (length(alt) ? alt + " " | "") + "<" + node["src"] + ">";
    elseif (kind == 'code_block)
      return $document_renderer:_code(node, format, options);
    elseif (kind == 'thematic_break)
      return html ? "<hr>" | "---";
    elseif (kind == 'list)
      return $document_renderer:_list(source, node, format, path);
    elseif (kind == 'definition_list)
      return $document_renderer:_definitions(node["items"], format, path);
    elseif (kind == 'table)
      return $document_renderer:_table(source, node, format, options, path);
    endif
    const body = $document_renderer:_join(node["children"],
      kind == 'block_quote ? (html ? "\n" | "\n\n") | "", format, path);
    if (kind == 'paragraph)
      return html ? "<p>" + body + "</p>" | body;
    elseif (kind == 'heading)
      if (html)
        const tag = "h" + tostr(node["level"]);
        return "<" + tag + ">" + body + "</" + tag + ">";
      endif
      return djot ? $document_renderer:_repeat("#", node["level"]) + " " + body | body;
    elseif (kind == 'block_quote)
      return html ? "<blockquote>" + body + "</blockquote>"
        | $document_renderer:_prefix(body, "> ", "> ");
    elseif (kind in {'emphasis, 'strong, 'strike})
      if (html)
        const tag = ['emphasis -> "em", 'strong -> "strong", 'strike -> "del"][kind];
        return "<" + tag + ">" + body + "</" + tag + ">";
      elseif (djot && length(body))
        const marker = ['emphasis -> "_", 'strong -> "*", 'strike -> "-"][kind];
        return "{" + marker + body + marker + "}";
      endif
      return body;
    elseif (kind == 'reference)
      if (html && interactions)
        return "<span data-moor-annotation=\"" + node["anchor"] + "\">" + body + "</span>";
      elseif (djot && interactions)
        return "[" + body + "]{annotation=" + node["anchor"] + "}";
      endif
      return body;
    elseif (kind == 'link)
      interactions || return body;
      if (html)
        return "<a href=\"" + $document_renderer:_escape(node["url"], format) + "\""
          + $document_renderer:_title(node, format) + ">" + body + "</a>";
      elseif (djot)
        return "[" + body + "](" + $document_renderer:_destination(node["url"])
          + ")" + $document_renderer:_title(node, format);
      endif
      return strcmp(body, node["url"]) == 0 ? body | body + " <" + node["url"] + ">";
    endif
    raise(E_INVARG, tostr(format) + " " + path + ": unsupported node");
  endmethod

  method _join owner: HACKER
    "Join serialized children while enforcing the body bound at every append.";
    const {parts, separator, format, path} = args;
    let body = "";
    for i in [1..length(parts)]
      body = body + (i > 1 ? separator | "") + parts[i];
      length(binary_from_str(body)) <= 524288
        || raise(E_QUOTA, tostr(format) + " " + path + ": rendered body exceeds 512 KiB");
    endfor
    return body;
  endmethod

  method _escape owner: HACKER
    "Escape literal text once. Plain output never executes an embedded ANSI escape.";
    let {text, format} = args;
    if (format == 'text_html)
      for pair in ({{"&", "&amp;"}, {"<", "&lt;"}, {">", "&gt;"}, {"\"", "&quot;"}, {"'", "&#39;"}})
        text = strsub(text, pair[1], pair[2], 1);
      endfor
      return text;
    endif
    text = strsub(text, binary_to_str(b"Gw=="), "\\x1b", 1);
    if (format == 'text_djot)
      const punctuation = "\\!\"#$%&'()*+,-./:;<=>?@[]^_`{|}~";
      for i in [1..length(punctuation)]
        text = strsub(text, punctuation[i], "\\" + punctuation[i], 1);
      endfor
    endif
    return text;
  endmethod

  method _destination owner: HACKER
    "Escape Djot link-destination delimiters without changing the URL.";
    let {url} = args;
    for character in ({"\\", "(", ")", "|"})
      url = strsub(url, character, "\\" + character, 1);
    endfor
    return url;
  endmethod

  method _title owner: HACKER
    "Serialize an optional title attribute, including quotes and backslashes.";
    const {node, format} = args;
    maphaskey(node, "title") || return "";
    if (format == 'text_html)
      return " title=\"" + $document_renderer:_escape(node["title"], format) + "\"";
    endif
    let title = strsub(node["title"], "\\", "\\\\", 1);
    title = strsub(title, "\"", "\\\"", 1);
    title = strsub(title, "|", "\\|", 1);
    return "{title=\"" + title + "\"}";
  endmethod

  method _repeat owner: HACKER
    "Repeat a bounded string for indentation, fences, or padding.";
    let {text, count} = args;
    (typeof(text) == TYPE_STR && typeof(count) == TYPE_INT && count >= 0)
      || raise(E_INVARG, "render.repeat: invalid text or count");
    (count <= 524288 && length(binary_from_str(text)) * count <= 524288)
      || raise(E_QUOTA, "render.repeat: output exceeds 512 KiB");
    let result = "";
    while (count > 0)
      count % 2 == 1 && (result = result + text);
      count = count / 2;
      count > 0 && (text = text + text);
    endwhile
    return result;
  endmethod

  method _fence owner: HACKER
    "Choose a backtick fence longer than every run in the opaque source.";
    const {text, minimum} = args;
    const bytes = binary_from_str(text);
    let run = 0;
    let longest = minimum - 1;
    for i in [1..length(bytes)]
      if (bytes[i] == 96)
        run = run + 1;
        longest = max(longest, run);
      else
        run = 0;
      endif
    endfor
    return $document_renderer:_repeat("`", longest + 1);
  endmethod

  method _inline_code owner: HACKER
    "Keep inline code opaque, with safe Djot fences and boundary padding.";
    let {text, format} = args;
    if (format == 'text_html)
      return "<code>" + $document_renderer:_escape(text, format) + "</code>";
    elseif (format == 'text_plain)
      return $document_renderer:_escape(text, format);
    endif
    text = $document_renderer:_escape(text, 'text_plain);
    length(text) || return "";
    const fence = $document_renderer:_fence(text, 1);
    return fence + (text[1] == "`" ? " " | "") + text
      + (text[$] == "`" ? " " | "") + fence;
  endmethod

  method _prefix owner: HACKER
    "Indent every physical line without discarding empty lines or trailing newlines.";
    const {text, first, rest} = args;
    return first + strsub(text, "\n", "\n" + rest, 1);
  endmethod

  method _code owner: HACKER
    "Render exact source. Optional line numbers occupy a separate HTML gutter or text prefix.";
    const {node, format, options} = args;
    const text = node["text"];
    const numbered = options['code_line_numbers] && maphaskey(node, "first_line");
    let numbers = {};
    let listing = text;
    if (numbered && length(text))
      const lines = $str_proto:split(text, "\n");
      let rendered = {};
      for i in [1..length(lines)]
        const number = tostr(node["first_line"] + i - 1);
        if (i == length(lines) && lines[i] == "" && text[$] == "\n")
          rendered = {@rendered, ""};
          continue;
        endif
        numbers = {@numbers, number};
        rendered = {@rendered, number + " | " + lines[i]};
      endfor
      listing = $list_proto:join(rendered, "\n");
    endif
    if (format == 'text_html)
      let attribute = "";
      if (maphaskey(node, "language") && length(node["language"]))
        attribute = " class=\"language-" + $document_renderer:_escape(node["language"], format) + "\"";
      endif
      const code = "<pre><code" + attribute + ">"
        + $document_renderer:_escape(text, format) + "</code></pre>";
      if (numbered && length(numbers))
        return "<table><tbody><tr><td aria-hidden=\"true\" style=\"user-select:none\"><pre>"
          + $list_proto:join(numbers, "\n") + "</pre></td><td>" + code + "</td></tr></tbody></table>";
      endif
      return code;
    elseif (format == 'text_plain)
      return $document_renderer:_escape(listing, format);
    endif
    listing = $document_renderer:_escape(listing, 'text_plain);
    const fence = $document_renderer:_fence(listing, 3);
    const language = maphaskey(node, "language") ? node["language"] | "";
    const final_break = length(listing) && listing[$] == "\n" ? "" | "\n";
    return fence + language + "\n" + listing + final_break + fence;
  endmethod
  method _list owner: HACKER
    "Render list items with block separation, nesting, numbering, and read-only task state.";
    const {source, node, format, path} = args;
    const html = format == 'text_html;
    const numbered = node["style"] == "numbered";
    const columns = format != 'text_plain && $document_renderer:_reference_list(source);
    const start = maphaskey(node, "start") ? node["start"] | 1;
    if (format == 'text_djot && numbered && start < 0)
      let definitions = {};
      for ordinal in [1..length(node["items"])]
        const item = node["items"][ordinal];
        let blocks = item["blocks"];
        if (maphaskey(item, "checked"))
          const check = item["checked"] ? "[x] " | "[ ] ";
          blocks = length(blocks) ? {check + blocks[1], @blocks[2..$]} | {check};
        endif
        definitions = {@definitions, ["term" -> {
          $document_renderer:_escape(tostr(start + ordinal - 1) + ".", format)
        }, "definition" -> blocks]};
      endfor
      return $document_renderer:_definitions(definitions, format, path);
    endif
    let items = {};
    for i in [1..length(node["items"])]
      const item = node["items"][i];
      const body = $document_renderer:_join(item["blocks"], html ? "\n" | "\n\n", format, path);
      let checkbox = "";
      if (maphaskey(item, "checked"))
        if (html)
          checkbox = "<input type=\"checkbox\" disabled" + (item["checked"] ? " checked" | "") + "> ";
        else
          checkbox = item["checked"] ? "[x] " | "[ ] ";
        endif
      endif
      if (html)
        items = {@items, "<li>" + checkbox + body + "</li>"};
      else
        const marker = numbered ? tostr(start + i - 1) + ". " | "- ";
        items = {@items, $document_renderer:_prefix(checkbox + body, marker,
          $document_renderer:_repeat(" ", length(marker)))};
      endif
    endfor
    const body = $document_renderer:_join(items, html ? "\n" | "\n\n", format, path);
    if (html)
      const tag = numbered ? "ol" | "ul";
      const start_attribute = numbered ? " start=\"" + tostr(start) + "\"" | "";
      const layout = columns ? " class=\"reference-columns\"" | "";
      return "<" + tag + start_attribute + layout + ">" + body + "</" + tag + ">";
    endif
    return (columns ? "{.reference-columns}\n" | "") + body;
  endmethod

  method _reference_list owner: HACKER
    "Use compact columns for reference lists, including adjacent command and source links.";
    const {node} = args;
    node["style"] == "bullet" && length(node["items"]) || return false;
    for item in (node["items"])
      if (maphaskey(item, "checked") || length(item["blocks"]) != 1)
        return false;
      endif
      const block = item["blocks"][1];
      if (block["kind"] != "paragraph")
        return false;
      endif
      let has_reference = false;
      for child in (block["children"])
        if (child["kind"] == "reference")
          has_reference = true;
        elseif (child["kind"] != "text" || length(child["text"]:trim()))
          return false;
        endif
      endfor
      has_reference || return false;
    endfor
    return true;
  endmethod

  method _definitions owner: HACKER
    "Keep definition terms and all definition blocks in order in every output format.";
    const {definitions, format, path} = args;
    let parts = {};
    for definition in (definitions)
      const term = $document_renderer:_join(definition["term"], "", format, path);
      const body = $document_renderer:_join(definition["definition"],
        format == 'text_html ? "\n" | "\n\n", format, path);
      if (format == 'text_html)
        parts = {@parts, "<dt>" + term + "</dt><dd>" + body + "</dd>"};
      elseif (format == 'text_djot)
        parts = {@parts, $document_renderer:_prefix(term, ": ", "  ") + "\n\n"
          + $document_renderer:_prefix(body, "  ", "  ")};
      elseif (index(body, "\n"))
        parts = {@parts, term + ":\n" + $document_renderer:_prefix(body, "  ", "  ")};
      else
        parts = {@parts, term + ": " + body};
      endif
    endfor
    const body = $document_renderer:_join(parts, format == 'text_html ? "\n" | "\n\n", format, path);
    return format == 'text_html ? "<dl>" + body + "</dl>" | body;
  endmethod

  method _single_line owner: HACKER
    "Check whether canonical inline content fits one physical table line without losing breaks.";
    const {inlines} = args;
    let pending = inlines;
    while (length(pending))
      const node = pending[$];
      pending = pending[1..$ - 1];
      node["kind"] in {"soft_break", "hard_break"} && return false;
      for key in ({"text", "alt", "title"})
        if (maphaskey(node, key))
          (index(node[key], "\n") || index(node[key], "\r") || index(node[key], "\t")) && return false;
        endif
      endfor
      if (maphaskey(node, "children"))
        pending = {@pending, @node["children"]};
      endif
    endwhile
    return true;
  endmethod

  method _simple_table owner: HACKER
    "A pipe table requires single-paragraph cells and single-line headers.";
    const {table} = args;
    if (maphaskey(table, "header"))
      for header in (table["header"])
        $document_renderer:_single_line(header) || return false;
      endfor
    endif
    for row in (table["rows"])
      for cell in (row["cells"])
        length(cell["blocks"]) == 1 || return false;
        const paragraph = cell["blocks"][1];
        paragraph["kind"] == "paragraph" || return false;
        $document_renderer:_single_line(paragraph["children"]) || return false;
      endfor
    endfor
    return true;
  endmethod

  method _table owner: HACKER
    "Use native HTML tables, safe Djot pipe tables, or complete labeled rows for complex cells.";
    const {source, table, format, options, path} = args;
    format == 'text_html && return $document_renderer:_html_table(table, path);
    let rows = {};
    const has_header = maphaskey(table, "header");
    if (has_header)
      rows = {{ $document_renderer:_join(header, "", format, path) for header in (table["header"]) }};
    endif
    const simple = $document_renderer:_simple_table(source);
    if (simple)
      for row in (table["rows"])
        rows = {@rows, {cell["blocks"][1] for cell in (row["cells"])}};
      endfor
      const caption = $document_renderer:_join(table["caption"], "", format, path);
      if (format == 'text_djot)
        let lines = {};
        for i in [1..length(rows)]
          lines = {@lines, "| " + $list_proto:join(rows[i], " | ") + " |"};
          if (has_header && i == 1)
            const alignments = ["start" -> ":---", "center" -> ":---:", "end" -> "---:"];
            lines = {@lines, "|" + $list_proto:join(
              {alignments[column["align"]] for column in (table["columns"])}, "|") + "|"};
          endif
        endfor
        const body = $document_renderer:_join(lines, "\n", format, path);
        return length(caption) ? caption + (length(body) ? "\n\n" + body | "") | body;
      endif
      const aligned = $document_renderer:_plain_table(rows, table["columns"], options['width], path);
      if (typeof(aligned) == TYPE_STR)
        return length(caption) ? caption + (length(aligned) ? "\n\n" + aligned | "") | aligned;
      endif
    endif
    return $document_renderer:_table_rows(source, table, format, options, path);
  endmethod

  method _html_table owner: HACKER
    "Serialize captions, headers, and all cell blocks as an HTML table.";
    const {table, path} = args;
    const alignments = ["start" -> "left", "center" -> "center", "end" -> "right"];
    const caption = $document_renderer:_join(table["caption"], "", 'text_html, path);
    let parts = length(caption) ? {"<caption>" + caption + "</caption>"} | {};
    if (maphaskey(table, "header"))
      let headers = {};
      for i in [1..length(table["columns"])]
        const label = $document_renderer:_join(table["header"][i], "", 'text_html, path);
        headers = {@headers, "<th scope=\"col\" align=\"" + alignments[table["columns"][i]["align"]]
          + "\">" + label + "</th>"};
      endfor
      parts = {@parts, "<thead><tr>" + $list_proto:join(headers, "") + "</tr></thead>"};
    endif
    let rows = {};
    for row in (table["rows"])
      let cells = {};
      for i in [1..length(row["cells"])]
        const body = $document_renderer:_join(row["cells"][i]["blocks"], "\n", 'text_html, path);
        cells = {@cells, "<td align=\"" + alignments[table["columns"][i]["align"]] + "\">" + body + "</td>"};
      endfor
      rows = {@rows, "<tr>" + $list_proto:join(cells, "") + "</tr>"};
    endfor
    parts = {@parts, "<tbody>" + $document_renderer:_join(rows, "\n", 'text_html, path) + "</tbody>"};
    return "<table>" + $document_renderer:_join(parts, "\n", 'text_html, path) + "</table>";
  endmethod

  method _plain_table owner: HACKER
    "Align ASCII cells that fit the requested width. Return false when labeled rows are needed.";
    const {rows, columns, width, path} = args;
    let widths = {0 for column in (columns)};
    for row in (rows)
      for i in [1..length(row)]
        const cell = row[i];
        length(binary_from_str(cell)) == length(cell) || return false;
        (index(cell, "\n") || index(cell, "\r") || index(cell, "\t")) && return false;
        widths[i] = max(widths[i], length(cell));
      endfor
    endfor
    let total = max(0, (length(columns) - 1) * 2);
    for column_width in (widths)
      total = total + column_width;
    endfor
    total <= width || return false;
    let lines = {};
    for row in (rows)
      let cells = {};
      for i in [1..length(row)]
        const padding = widths[i] - length(row[i]);
        const alignment = columns[i]["align"];
        const left = alignment == "end" ? padding | (alignment == "center" ? padding / 2 | 0);
        cells = {@cells, $document_renderer:_repeat(" ", left) + row[i]
          + $document_renderer:_repeat(" ", padding - left)};
      endfor
      lines = {@lines, $list_proto:join(cells, "  ")};
    endfor
    return $document_renderer:_join(lines, "\n", 'text_plain, path);
  endmethod

  method _table_rows owner: HACKER
    "Render all cell blocks as labeled row groups, emitting interactive headers only once.";
    const {source, table, format, options, path} = args;
    let parts = {};
    const caption = $document_renderer:_join(table["caption"], "", format, path);
    length(caption) && (parts = {@parts, caption});
    let labels = {};
    if (maphaskey(table, "header"))
      let headings = {};
      let interactive = false;
      for i in [1..length(source["header"])]
        let label_parts = {};
        for inline in (source["header"][i])
          const {label, ignored} = $document_renderer:_walk(inline, 'inline, format, options,
            ['annotations -> [], 'next_anchor -> 1], path + ".header", false);
          label_parts = {@label_parts, label};
        endfor
        const label = $document_renderer:_join(label_parts, "", format, path);
        const heading = $document_renderer:_join(table["header"][i], "", format, path);
        labels = {@labels, label};
        headings = {@headings, heading};
        strcmp(label, heading) != 0 && (interactive = true);
      endfor
      if (interactive || !length(table["rows"]))
        parts = {@parts, "Columns: " + $list_proto:join(headings, ", ")};
      endif
    else
      labels = {"Column " + tostr(i) for i in [1..length(table["columns"]) ]};
    endif
    for i in [1..length(table["rows"])]
      let definitions = {};
      for j in [1..length(labels)]
        definitions = {@definitions, ["term" -> {labels[j]},
          "definition" -> table["rows"][i]["cells"][j]["blocks"]]};
      endfor
      parts = {@parts, "Row " + tostr(i), $document_renderer:_definitions(definitions, format, path)};
    endfor
    return $document_renderer:_join(parts, "\n\n", format, path);
  endmethod
endobject
