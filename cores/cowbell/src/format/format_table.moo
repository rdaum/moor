object FORMAT_TABLE [
  import_export_id -> "format_table",
  import_export_hierarchy -> {"format"}
]
  name: "Table Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for table content in events.";

  method mk owner: HACKER
    "Create a table flyweight from header cells and rows of cells.";
    const {headers, rows} = args;
    typeof(headers) == TYPE_LIST || raise(E_TYPE, "Headers must be a list");
    typeof(rows) == TYPE_LIST || raise(E_TYPE, "Rows must be a list");
    return toflyweight(this, ['headers -> headers, 'rows -> rows]);
  endmethod

  method compose owner: HACKER
    "Compose cells in order, then render the table with their combined annotations.";
    const {render_for, content_type, event} = args;
    const {headers, header_annotations} =
      $format:compose_parts(this.headers, render_for, content_type, event);
    let annotations = header_annotations;
    let rows = {};
    for raw_row in (this.rows)
      const {row, row_annotations} =
        $format:compose_parts(raw_row, render_for, content_type, event);
      rows = {@rows, row};
      annotations = $format:merge_annotations(annotations, row_annotations);
    endfor
    if (!headers || !rows)
      return content_type == 'text_html ? <$html, {"p", {}, {"(empty table)"}}> | "(empty table)";
    endif
    if (content_type == 'text_html)
      return $format:result(this:_render_html(headers, rows), annotations);
    elseif (content_type == 'text_djot)
      return $format:result(this:_render_djot(headers, rows), annotations);
    endif
    return $format:result(this:_render_plain(headers, rows), annotations);
  endmethod

  method _render_html owner: HACKER
    "Build an HTML table tree from composed headers and rows.";
    const {headers, rows} = args;
    const header_cells = {<$html, {"th", {}, {header}}> for header in (headers)};
    const head = <$html, {"thead", {}, {<$html, {"tr", {}, header_cells}>}}>;
    let body_rows = {};
    for row_index in [1..length(rows)]
      if (row_index % 50 == 0)
        suspend_if_needed();
      endif
      const cells = {<$html, {"td", {}, {cell}}> for cell in (rows[row_index])};
      body_rows = {@body_rows, <$html, {"tr", {}, cells}>};
    endfor
    return <$html, {"table", {}, {head, <$html, {"tbody", {}, body_rows}>}}>;
  endmethod

  method _render_djot owner: HACKER
    "Join composed cells into an unpadded Djot pipe table.";
    const {headers, rows} = args;
    fn pipe_row(cells)
      return "| " + {tostr(cell) for cell in (cells)}:join(" | ") + " |";
    endfn
    const separator = "| " + {"-" for header in (headers)}:join(" | ") + " |";
    let lines = {pipe_row(headers), separator};
    for row_index in [1..length(rows)]
      if (row_index % 50 == 0)
        suspend_if_needed();
      endif
      lines = {@lines, pipe_row(rows[row_index])};
    endfor
    return lines:join("\n");
  endmethod

  method _render_plain owner: HACKER
    "Align composed cells in header columns. Fill missing cells and omit extra ones.";
    const {headers, rows} = args;
    fn padded_row(cells, widths)
      return {$str_proto:pad_right(i <= length(cells) ? tostr(cells[i]) | "", widths[i])
        for i in [1..length(widths)]}:join(" | ");
    endfn
    let widths = {length(tostr(header)) for header in (headers)};
    for row_index in [1..length(rows)]
      if (row_index % 50 == 0)
        suspend_if_needed();
      endif
      const row = rows[row_index];
      for i in [1..min(length(row), length(widths))]
        widths[i] = max(widths[i], length(tostr(row[i])));
      endfor
    endfor
    const separator = {$str_proto:space(width, "-") for width in (widths)}:join("-+-");
    let lines = {padded_row(headers, widths), separator};
    for row_index in [1..length(rows)]
      if (row_index % 50 == 0)
        suspend_if_needed();
      endif
      lines = {@lines, padded_row(rows[row_index], widths)};
    endfor
    return lines:join("\n");
  endmethod

  method test_simple_table owner: HACKER
    "HTML preserves the header and every row and cell in order.";
    const table = this:mk({"Name", "Age"}, {{"Alice", "25"}, {"Bob", "30"}});
    const html = table:compose($nothing, 'text_html, $nothing);
    const expected = {"table", {},
      {"thead", {}, {"tr", {}, {"th", {}, "Name"}, {"th", {}, "Age"}}},
      {"tbody", {},
        {"tr", {}, {"td", {}, "Alice"}, {"td", {}, "25"}},
        {"tr", {}, {"td", {}, "Bob"}, {"td", {}, "30"}}}};
    $test_utils:assert_eq(html:to_xml_tag(), expected, "HTML table structure");
    return true;
  endmethod

  method test_plain_text_table owner: HACKER
    "Plain text aligns columns and pads missing cells without adding extra columns.";
    const table = this:mk({"Item", "Price"},
      {{"Apple", "$1.00"}, {"Banana"}, {"Pear", "$0.50", "extra"}});
    const text = table:compose($nothing, 'text_plain, $nothing);
    const expected = "Item   | Price\n-------+------\nApple  | $1.00\nBanana |      \nPear   | $0.50";
    $test_utils:assert_eq(strcmp(text, expected), 0, "Plain table spacing");
    return true;
  endmethod

  method test_djot_table owner: HACKER
    "Djot preserves the supplied cells without column padding.";
    const table = this:mk({"Item", "Price"},
      {{"Apple", "$1.00"}, {"Banana"}, {"Pear", "$0.50", "extra"}});
    const text = table:compose($nothing, 'text_djot, $nothing);
    const expected = "| Item | Price |\n| - | - |\n| Apple | $1.00 |\n"
      + "| Banana |\n| Pear | $0.50 | extra |";
    $test_utils:assert_eq(strcmp(text, expected), 0, "Djot table spacing");
    return true;
  endmethod

  method test_empty_table owner: HACKER
    "Tables without headers or rows use the empty-table placeholder in each format.";
    for cells in ({{{}, {}}, {{"Header"}, {}}, {{}, {{"Cell"}}}})
      const table = this:mk(@cells);
      const html = table:compose($nothing, 'text_html, $nothing);
      $test_utils:assert_eq(html:to_xml_tag(), {"p", {}, "(empty table)"}, "Empty HTML table");
      for format in ({'text_plain, 'text_djot})
        $test_utils:assert_eq(table:compose($nothing, format, $nothing), "(empty table)",
          "Empty text table");
      endfor
    endfor
    return true;
  endmethod
endobject
