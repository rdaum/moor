object DOCUMENT_SCENARIOS
  name: "Document Contract Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method _text owner: ARCH_WIZARD
    "Construct literal text for these scenarios.";
    return $document_node:mk('text, {}, ['text -> args[1]]);
  endmethod

  method _paragraph owner: ARCH_WIZARD
    "Construct a paragraph from explicit inline nodes.";
    return $document_node:mk('paragraph, args[1]);
  endmethod

  method _reference owner: ARCH_WIZARD
    "Use an opaque, deliberately nonexistent identity without reading an object.";
    const {label} = args;
    return $document_node:mk('reference, {this:_text(label)},
      ['descriptor -> ['kind -> 'object, 'ref -> "oid:987654321"]]);
  endmethod

  method _invalid owner: ARCH_WIZARD
    "Require a structural or resource error identifying the expected path or rule.";
    const {document, fragment, ?code = E_INVARG} = args;
    try
      $document:encode(document);
    except error (E_INVARG, E_QUOTA)
      $test_utils:assert_eq(error[1], code, "Validation error kind");
      $test_utils:assert_true(index(error[2], fragment, 1), "Validation error identifies its location");
      return true;
    endtry
    raise(E_ASSERT, "Expected invalid document: " + fragment);
  endmethod

  method test_empty_and_roundtrip owner: ARCH_WIZARD
    "Empty documents have one uniform render shape and an inert versioned encoding.";
    const document = $document:mk({});
    const encoded = $document:encode(document);
    $test_utils:assert_eq(encoded, ["version" -> 1, "blocks" -> {}], "Empty encoding");
    $test_utils:assert_eq($document:decode(parse_json(generate_json(encoded))), document, "JSON roundtrip");
    for format in ({'text_html, 'text_djot, 'text_plain})
      $test_utils:assert_eq($document:render(document, format),
        ['content_type -> format, 'body -> "", 'annotations -> []], "Empty render result");
    endfor
    return true;
  endmethod

  method test_literal_paragraphs owner: ARCH_WIZARD
    "Literal punctuation stays literal, and paragraph boundaries survive every serializer.";
    const document = $document:mk({
      this:_paragraph({this:_text("A *literal* <&> sign.")}),
      this:_paragraph({this:_text("Next.")})
    });
    $test_utils:assert_eq($document:render(document, 'text_html)['body],
      "<p>A *literal* &lt;&amp;&gt; sign.</p>\n<p>Next.</p>", "HTML escaping and block boundaries");
    $test_utils:assert_eq($document:render(document, 'text_djot)['body],
      "A \\*literal\\* \\<\\&\\> sign\\.\n\nNext\\.", "Djot literal punctuation and paragraphs");
    $test_utils:assert_eq($document:render(document, 'text_plain)['body],
      "A *literal* <&> sign.\n\nNext.", "Plain paragraphs");
    return true;
  endmethod

  method test_repeated_references owner: ARCH_WIZARD
    "References are inert identities. Each visible occurrence gets a deterministic local anchor.";
    const reference = this:_reference("A <key> *literal*");
    const document = $document:mk({this:_paragraph({reference, this:_text(" and "), reference})});
    const encoded = $document:encode(document);
    $test_utils:assert_eq($document:encode($document:decode(encoded)), encoded, "Reference metadata roundtrip");
    for format in ({'text_html, 'text_djot})
      const result = $document:render(document, format);
      $test_utils:assert_eq(mapkeys(result['annotations]), {"a1", "a2"}, "Local occurrence anchors");
      $test_utils:assert_eq(result['annotations]["a1"],
        ["kind" -> "object", "ref" -> "oid:987654321"], "No object lookup during rendering");
      $test_utils:assert_eq($document:render(document, format), result, "Deterministic serialization");
    endfor
    const plain = $document:render(document, 'text_plain);
    $test_utils:assert_eq(plain['annotations], [], "Plain text has no markers");
    $test_utils:assert_eq(plain['body], "A <key> *literal* and A <key> *literal*", "Reference labels");
    return true;
  endmethod

  method test_code_source_and_gutters owner: ARCH_WIZARD
    "Code retains spaces and delimiters. Display numbering does not change encoded source.";
    const text = " before\n```\nafter  ";
    const code = $document_node:mk('code_block, {}, ['text -> text, 'language -> "moo", 'first_line -> 40]);
    const document = $document:mk({code});
    const djot = $document:render(document, 'text_djot, ['code_line_numbers -> false]);
    $test_utils:assert_eq(djot['body], "````moo\n" + text + "\n````", "Collision-free fence");
    $test_utils:assert_eq($document:render(document, 'text_plain)['body],
      "40 |  before\n41 | ```\n42 | after  ", "Source line origin");
    const html = $document:render(document, 'text_html)['body];
    $test_utils:assert_true(index(html, "<code class=\"language-moo\">" + text + "</code>", 1),
      "HTML code text excludes the gutter");
    $test_utils:assert_eq($document:encode(document)["blocks"][1]["text"], text, "Exact source in metadata");
    const inline = $document_node:mk('inline_code, {}, ['text -> "` a `` b `"]);
    const inline_document = $document:mk({this:_paragraph({inline})});
    $test_utils:assert_eq($document:render(inline_document, 'text_djot)['body],
      "``` ` a `` b ` ```", "Inline fence and boundary padding");
    server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
      ["case" -> "code-fence", "format" -> "text_djot", "body" -> djot['body]]));
    server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
      ["case" -> "inline-code", "format" -> "text_djot",
        "body" -> $document:render(inline_document, 'text_djot)['body]]));
    return true;
  endmethod

  method test_invalid_tree_paths owner: ARCH_WIZARD
    "Reject foreign shapes and inherited defaults before rendering or serialization.";
    this:_invalid($document:mk({this:_text("root inline")}), "document.blocks[0]");
    this:_invalid($document:mk({this:_paragraph({this:_paragraph({})})}), "document.blocks[0].children[0]");
    this:_invalid(<$document, .version = 2, {}>, "document.version");
    this:_invalid(<$document, {}>, "document.version");
    this:_invalid($document:mk({<$document_node, .kind = 'raw_html, .text = "<b>no</b>">}), "unknown node kind");
    this:_invalid($document:mk({<$document_node, .kind = 'paragraph, .class = "raw", {}>}), "unknown field");
    this:_invalid($document:mk({<$root, .kind = 'paragraph, {}>}), "wrong document delegate");
    this:_invalid($document:mk({this:_paragraph({this:_text("one\ntwo")})}), ".text");
    const inner = $document_node:mk('strong, {this:_reference("inner")});
    const outer = $document_node:mk('link, {inner}, ['url -> "https://example.org/"]);
    this:_invalid($document:mk({this:_paragraph({outer})}), "interactive labels cannot nest");
    const unsafe = $document_node:mk('link, {this:_text("run")}, ['url -> "javascript:alert(1)"]);
    this:_invalid($document:mk({this:_paragraph({unsafe})}), "unsupported URL scheme");
    return true;
  endmethod

  method test_descriptor_finalization owner: ARCH_WIZARD
    "Finalization removes invalid interaction data while retaining formatted labels in every format.";
    const label = $document_node:mk('strong, {this:_text("Keep this label")});
    const invalid = $document_node:mk('reference, {label},
      ['descriptor -> ['kind -> 'object, 'ref -> #2]]);
    const document = $document:mk({this:_paragraph({invalid})});
    this:_invalid(document, ".descriptor.ref");
    const prepared = $document:prepare(document);
    $test_utils:assert_eq(length(prepared['diagnostics]), 1, "Invalid descriptor diagnostic");
    $test_utils:assert_eq(prepared['encoded]["blocks"][1]["children"][1]["kind"], "strong",
      "The descriptor is absent from canonical metadata");
    for format in ({'text_html, 'text_djot, 'text_plain})
      const rendered = $document:render(prepared['document], format);
      $test_utils:assert_eq(rendered['annotations], [], "No format can reactivate the reference");
      $test_utils:assert_true(index(rendered['body], "Keep this label"), "Formatted label retained");
    endfor
    return true;
  endmethod
  method _table owner: ARCH_WIZARD
    "Construct a two-column table with caller-supplied block cells and headers.";
    const {cells, ?header = false, ?caption = {}} = args;
    let fields = ['columns -> {['align -> 'start], ['align -> 'end]}, 'caption -> caption];
    typeof(header) == TYPE_LIST && (fields['header] = header);
    return $document_node:mk('table, {['cells -> {['blocks -> cells[1]], ['blocks -> cells[2]]}]}, fields);
  endmethod

  method test_lists_definitions_and_inline_nodes owner: ARCH_WIZARD
    "All prose nodes retain their text and nesting, including task states and numbered lists.";
    const terms = $document_node:mk('definition_list, {
      ['term -> {this:_text("Key")}, 'definition -> {
        this:_paragraph({this:_text("First definition paragraph.")}),
        this:_paragraph({this:_text("Second definition paragraph.")})
      }]
    });
    const nested = $document_node:mk('list, {
      ['checked -> false, 'blocks -> {this:_paragraph({this:_text("Waiting")})}],
      ['checked -> true, 'blocks -> {this:_paragraph({this:_text("Done")})}]
    }, ['style -> 'bullet]);
    const list = $document_node:mk('list, {
      ['blocks -> {this:_paragraph({this:_text("First item")}), nested}],
      ['blocks -> {terms}]
    }, ['style -> 'numbered, 'start -> 3]);
    const styled = this:_paragraph({
      $document_node:mk('emphasis, {this:_text("gentle")}),
      $document_node:mk('soft_break),
      $document_node:mk('strong, {this:_text("strong")}),
      $document_node:mk('hard_break),
      $document_node:mk('strike, {this:_text("removed")}), this:_text(" "),
      $document_node:mk('link, {this:_text("Guide")},
        ['url -> "https://example.org/a(b)?x=1&y=2", 'title -> "A \"title\""]),
      $document_node:mk('image, {},
        ['src -> "https://example.org/bell.png", 'alt -> "A *brass* bell", 'title -> "Bell"])
    });
    const document = $document:mk({
      $document_node:mk('heading, {this:_text("Guide")}, ['level -> 2]),
      $document_node:mk('block_quote, {styled}), list, $document_node:mk('thematic_break)
    });
    const encoded = $document:encode(document);
    $test_utils:assert_eq($document:encode($document:decode(encoded)), encoded, "Whole tree roundtrip");
    for format in ({'text_html, 'text_djot, 'text_plain})
      const result = $document:render(document, format);
      for phrase in ({"gentle", "strong", "removed", "Waiting", "Done", "Key", "First", "Second"})
        $test_utils:assert_true(index(result['body], phrase), "Every nested value remains readable");
      endfor
      server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
        ["case" -> "prose", "format" -> tostr(format), "body" -> result['body]]));
    endfor
    const html = $document:render(document, 'text_html)['body];
    for marker in ({"<h2>", "<blockquote>", "<em>", "<strong>", "<del>", "<br>",
      "<ol start=\"3\">", "<ul>", "disabled checked", "<dl>", "<dd>", "<hr>", "<img "})
      $test_utils:assert_true(index(html, marker, 1), "HTML structural element");
    endfor
    return true;
  endmethod

  method test_simple_and_complex_tables owner: ARCH_WIZARD
    "Tables preserve rectangular data, alignment, multiple blocks, and reference occurrences.";
    const header = {{this:_reference("Item")}, {this:_text("Details")}};
    const simple = this:_table({
      {this:_paragraph({this:_reference("brass | key")})},
      {this:_paragraph({this:_text("42")})}
    }, header, {this:_reference("Inventory")});
    const complex = this:_table({
      {this:_paragraph({this:_reference("brass key")})},
      {this:_paragraph({this:_text("Opens the workshop.")}),
        this:_paragraph({this:_reference("Inspect the key")})}
    }, header);
    for example in ({{"simple-table", simple}, {"complex-table", complex}})
      const document = $document:mk({example[2]});
      for format in ({'text_html, 'text_djot, 'text_plain})
        const result = $document:render(document, format);
        $test_utils:assert_eq(length(result['annotations]), format == 'text_plain ? 0 | 3,
          "Table references occur once in reading order");
        server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
          ["case" -> example[1], "format" -> tostr(format), "body" -> result['body],
            "annotations" -> result['annotations]]));
      endfor
    endfor
    const djot = $document:render($document:mk({simple}), 'text_djot)['body];
    $test_utils:assert_true(index(djot, "|:---|---:|", 1), "Djot header alignment");
    $test_utils:assert_true(index(djot, "brass \\| key", 1), "Literal table pipes are escaped");
    const fallback = $document:render($document:mk({complex}), 'text_djot)['body];
    $test_utils:assert_true(index(fallback, "Columns: [Item]{annotation=a1}", 1), "Header reference emitted once");
    $test_utils:assert_true(index(fallback, "Row 1\n\n: Item", 1), "Whole-table row fallback");
    $test_utils:assert_true(index(fallback, "  Opens the workshop\\.\n  \n  [Inspect the key]{annotation=a3}", 1),
      "Complex cell retains both paragraphs");
    const narrow = $document:render($document:mk({simple}), 'text_plain, ['width -> 5]);
    $test_utils:assert_true(index(narrow['body], "Row 1"), "Narrow terminal uses complete labeled rows");
    const invalid = $document_node:mk('table, {['cells -> {}]},
      ['columns -> {['align -> 'start]}, 'caption -> {}]);
    this:_invalid($document:mk({invalid}), "document.blocks[0].rows[0].cells");
    return true;
  endmethod

  method test_all_descriptor_kinds owner: ARCH_WIZARD
    "Every supported interaction survives inert encoding, including command templates and UUIDs.";
    const descriptors = {
      ['kind -> 'object, 'ref -> "uuid:abcdef-0123456789", 'object_kind -> 'person],
      ['kind -> 'help, 'provider -> "oid:91", 'topic -> "@changes"],
      ['kind -> 'verb, 'receiver -> "oid:73", 'name -> "look", 'definer -> "oid:1"],
      ['kind -> 'property, 'object -> "oid:73", 'name -> "description"],
      ['kind -> 'change, 'provider -> "oid:2000", 'review -> 1, 'generation -> 2, 'row -> "r1"],
      ['kind -> 'command, 'command -> "north", 'action -> ['label -> "Go north", 'icon -> "north"],
        'exit -> ['source -> "oid:1", 'destination -> "oid:2", 'passage -> "north"]],
      ['kind -> 'command, 'template -> "give {dobj} to {iobj}", 'arguments -> [
        'dobj -> ['label -> "item", 'expected_kind -> 'object, 'required -> true,
          'suggestions -> ['provider -> "oid:73", 'source -> "nearby"],
          'bound -> ['value -> "oid:74", 'label -> "key"]],
        'iobj -> ['label -> "recipient", 'expected_kind -> 'text, 'required -> false]
      ]]
    };
    const nodes = {$document_node:mk('reference, {this:_text("label")}, ['descriptor -> descriptor])
      for descriptor in (descriptors)};
    const document = $document:mk({this:_paragraph(nodes)});
    const encoded = $document:encode(document);
    $test_utils:assert_eq($document:encode($document:decode(parse_json(generate_json(encoded)))), encoded,
      "Descriptor JSON roundtrip");
    $test_utils:assert_eq(length($document:render(document, 'text_html)['annotations]), 7, "All descriptor kinds render");
    const bad = $document_node:mk('reference, {this:_text("bad")},
      ['descriptor -> ['kind -> 'command, 'template -> "say {dobj} {dobj}", 'arguments -> [
        'dobj -> ['label -> "message", 'expected_kind -> 'text]
      ]]]);
    this:_invalid($document:mk({this:_paragraph({bad})}), "repeated template placeholder");
    return true;
  endmethod

  method test_resource_limits owner: ARCH_WIZARD
    "Depth, record count, metadata bytes, and rendered bytes fail before returning oversized output.";
    let inline = this:_text("deep");
    for depth_index in [1..30]
      inline = $document_node:mk('strong, {inline});
    endfor
    const boundary = $document:mk({this:_paragraph({inline})});
    $document:validate(boundary);
    $test_utils:assert_eq($document:render(boundary, 'text_plain)['body], "deep", "Depth 32 is usable");
    this:_invalid($document:mk({this:_paragraph({$document_node:mk('strong, {inline})})}),
      "nesting exceeds", E_QUOTA);
    this:_invalid($document:mk({this:_paragraph({}) for record_index in [1..8193]}), "too many entries", E_QUOTA);
    const unicode = $document_renderer:_repeat("é", 131050);
    this:_invalid($document:mk({this:_paragraph({this:_text(unicode)})}), "encoded document", E_QUOTA);
    const escaped = $document_renderer:_repeat("<", 150000);
    const document = $document:mk({this:_paragraph({this:_text(escaped)})});
    $document:validate(document);
    try
      $document:render(document, 'text_html);
    except error (E_QUOTA)
      $test_utils:assert_true(index(error[2], "text_html document.blocks[0].children[0]", 1),
        "Render limit names format and node path");
      return true;
    endtry
    raise(E_ASSERT, "Escaped output must respect the rendered-byte limit");
  endmethod

  method test_reference_budget owner: ARCH_WIZARD
    "The 513th reference becomes its label before encoding and rendering any format.";
    const reference = this:_reference("x");
    const document = $document:mk({this:_paragraph({reference for i in [1..513]})});
    this:_invalid(document, "more than 512 references", E_QUOTA);
    const prepared = $document:prepare(document);
    $test_utils:assert_eq(length(prepared['diagnostics]), 1, "One reference exceeds the budget");
    const children = prepared['encoded]["blocks"][1]["children"];
    $test_utils:assert_eq(children[512]["kind"], "reference", "Last active reference");
    $test_utils:assert_eq(children[513], ["kind" -> "text", "text" -> "x"], "Excess label is inert metadata");
    const html = $document:render(prepared['document], 'text_html);
    $test_utils:assert_eq(length(html['annotations]), 512, "HTML follows the finalized budget");
    $test_utils:assert_eq(html['annotations]["a512"]["ref"], "oid:987654321", "Stable final anchor");
    $test_utils:assert_eq(length($document:render(prepared['document], 'text_plain)['body]), 513,
      "Every label remains readable");
    return true;
  endmethod
  method test_metadata_validation_and_options owner: ARCH_WIZARD
    "External metadata rejects case aliases, unknown fields, wrong types, and unsupported versions.";
    const document = $document:mk({this:_paragraph({this:_text("safe")})});
    const encoded = $document:encode(document);
    const invalid = {
      ["version" -> 2, "blocks" -> {}],
      ["Version" -> 1, "blocks" -> {}],
      ["version" -> 1, "blocks" -> {}, "html" -> "<b>bypass</b>"],
      ["version" -> 1, "blocks" -> {["kind" -> "Paragraph", "children" -> {}]}],
      ["version" -> 1, "blocks" -> {["kind" -> "paragraph", "children" -> {
        ["kind" -> "text", "text" -> #2]
      }]}],
      ["version" -> 1, "blocks" -> {["kind" -> "paragraph", "children" -> #2]}]
    };
    for value in (invalid)
      $test_utils:assert_raises(E_INVARG, $document, "decode", {value},
        "Malformed metadata is rejected: " + generate_json(value));
    endfor
    const aliased_descriptor = ["kind" -> "object", "ref" -> "oid:73", "object_kind" -> "person"];
    const aliased_reference = ["kind" -> "reference", "descriptor" -> aliased_descriptor,
      "children" -> {["kind" -> "text", "text" -> "label"]}];
    const aliased_document = ["version" -> 1, "blocks" -> {
      ["kind" -> "paragraph", "children" -> {aliased_reference}]
    }];
    $test_utils:assert_raises(E_INVARG, $document, "decode", {aliased_document},
      "Descriptor metadata rejects internal snake_case aliases");
    for options in ({['width -> 0], ['width -> 80.0], ['code_line_numbers -> 1], ['raw_html -> true]})
      $test_utils:assert_raises(E_INVARG, $document, "render", {document, 'text_plain, options},
        "Invalid rendering options are rejected");
    endfor
    $test_utils:assert_raises(E_INVARG, $document, "render", {document, "text/html"},
      "Core content types use symbols");
    $test_utils:assert_eq($document:encode(document), encoded, "Invalid calls do not change values");
    return true;
  endmethod

  method test_empty_cells_and_unicode_layout owner: ARCH_WIZARD
    "Empty values remain empty; complex and Unicode cells keep complete content in narrow output.";
    const table = this:_table({{}, {this:_paragraph({this:_text("雪")})}});
    const document = $document:mk({table});
    const plain = $document:render(document, 'text_plain)['body];
    $test_utils:assert_true(index(plain, "Column 1:"), "Absent headers receive column labels");
    $test_utils:assert_true(index(plain, "Column 2: 雪"), "Unicode survives table fallback");
    $test_utils:assert_true(!index(plain, "empty"), "Renderer invents no empty-state message");
    const empty_table = $document_node:mk('table, {},
      ['columns -> {['align -> 'start]}, 'caption -> {}]);
    for format in ({'text_plain, 'text_djot})
      $test_utils:assert_eq($document:render($document:mk({empty_table}), format)['body], "",
        "An empty table is not a status message");
    endfor
    const code = $document_node:mk('code_block, {}, ['text -> "first\n\nlast\n", 'first_line -> 7]);
    $test_utils:assert_eq($document:render($document:mk({code}), 'text_plain)['body],
      "7 | first\n8 | \n9 | last\n", "Numbering retains blank lines and the final newline");
    return true;
  endmethod

  method test_descriptor_unicode_size owner: ARCH_WIZARD
    "Descriptor size follows JSON UTF-16 units, not MOO character count or literal syntax.";
    const command = $document_renderer:_repeat("💡", 513);
    const reference = $document_node:mk('reference, {this:_text("Unicode label")},
      ['descriptor -> ['kind -> 'command, 'command -> command]]);
    this:_invalid($document:mk({this:_paragraph({reference})}), "descriptor text length");
    const quotes = $document_renderer:_repeat("\"", 1024);
    const oversized = $document_node:mk('reference, {this:_text("Quote label")},
      ['descriptor -> ['kind -> 'command, 'command -> quotes]]);
    const document = $document:mk({this:_paragraph({oversized})});
    this:_invalid(document, "descriptor exceeds 2048 UTF-16 units");
    const prepared = $document:prepare(document);
    $test_utils:assert_eq($document:render(prepared['document], 'text_plain)['body], "Quote label",
      "Oversized descriptors preserve their labels");
    return true;
  endmethod
  method test_numbered_list_starts owner: ARCH_WIZARD
    "Numbered lists default to one and retain explicit zero or negative starting values.";
    const items = {['blocks -> {this:_paragraph({this:_text("First")})}],
      ['blocks -> {this:_paragraph({this:_text("Second")})}]};
    const default_list = $document_node:mk('list, items, ['style -> 'numbered]);
    $test_utils:assert_eq($document:render($document:mk({default_list}), 'text_plain)['body],
      "1. First\n\n2. Second", "Default list start");
    for start in ({0, -2})
      const list = $document_node:mk('list, items, ['style -> 'numbered, 'start -> start]);
      const document = $document:mk({list});
      $test_utils:assert_true(index($document:render(document, 'text_html)['body],
        "<ol start=\"" + tostr(start) + "\">", 1), "HTML list origin");
      const djot = $document:render(document, 'text_djot)['body];
      server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
        ["case" -> "numbering-" + tostr(start), "format" -> "text_djot", "body" -> djot]));
    endfor
    return true;
  endmethod
  method test_record_budget_across_branches owner: ARCH_WIZARD
    "The node budget counts table records across separate arrays, including empty cells.";
    const columns = {['align -> 'start] for column_index in [1..4095]};
    const cells = {['blocks -> {}] for cell_index in [1..4095]};
    const table = $document_node:mk('table, {['cells -> cells]}, ['columns -> columns, 'caption -> {}]);
    const encoded = $document:encode($document:mk({table}));
    $test_utils:assert_eq(length(encoded["blocks"][1]["rows"][1]["cells"]), 4095,
      "Exactly 8192 records validate");
    this:_invalid($document:mk({table, $document_node:mk('thematic_break)}),
      "document.blocks[1]: document exceeds 8192 records", E_QUOTA);
    return true;
  endmethod

  method test_terminal_code_controls owner: ARCH_WIZARD
    "ANSI bytes in code become visible terminal text while encoded source remains exact.";
    const source = binary_to_str(b"Gw==") + "[31mred";
    const inline = $document_node:mk('inline_code, {}, ['text -> source]);
    const code = $document_node:mk('code_block, {}, ['text -> source]);
    const document = $document:mk({this:_paragraph({inline}), code});
    for format in ({'text_plain, 'text_djot})
      const body = $document:render(document, format)['body];
      $test_utils:assert_true(!index(body, binary_to_str(b"Gw==")), "No raw terminal escape");
      $test_utils:assert_true(index(body, "\\x1b[31mred", 1), "Visible escape spelling");
    endfor
    $test_utils:assert_eq($document:encode(document)["blocks"][2]["text"], source,
      "Original code remains available in metadata");
    return true;
  endmethod

  method test_table_link_delimiters owner: ARCH_WIZARD
    "Pipes in link destinations and titles do not introduce extra Djot table columns.";
    const link = $document_node:mk('link, {this:_text("link")},
      ['url -> "https://example.org/a|b", 'title -> "left|right"]);
    const table = this:_table({{this:_paragraph({link})}, {this:_paragraph({this:_text("second")})}});
    const body = $document:render($document:mk({table}), 'text_djot)['body];
    $test_utils:assert_true(index(body, "https://example.org/a\\|b", 1), "Destination pipe escape");
    $test_utils:assert_true(index(body, "title=\"left\\|right\"", 1), "Title pipe escape");
    server_log("DOCUMENT_RENDER_SAMPLE " + generate_json(
      ["case" -> "table-links", "format" -> "text_djot", "body" -> body]));
    return true;
  endmethod
endobject
