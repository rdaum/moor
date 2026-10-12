object DJOT_SCENARIOS
  name: "Djot Authoring Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  property providers (owner: HACKER, flags: "r") =
    {HELP_TOPICS, PROG_HELP_TOPICS, BUILDER_HELP_TOPICS, ADMIN_HELP_TOPICS, WIZARD_HELP_TOPICS};

  override import_export_hierarchy = {"tests", "baseline"};
  override import_export_id = "djot_scenarios";

  method _parse owner: HACKER
    "Return the encoded blocks for structural assertions.";
    const {source} = args;
    return $document:encode($document:mk($format.djot:blocks(source)))["blocks"];
  endmethod

  method test_djot_inline_and_rendering owner: HACKER
    "Authored styles become canonical nodes and render consistently across all core formats.";
    const nodes = $format.djot:inline("A *brass* key.");
    const document = $document:mk({$document_node:mk('paragraph, nodes)});
    const encoded = $document:encode(document);
    $test_utils:assert_eq(encoded["blocks"][1]["children"], {
      ["kind" -> "text", "text" -> "A "],
      ["kind" -> "strong", "children" -> {["kind" -> "text", "text" -> "brass"]}],
      ["kind" -> "text", "text" -> " key."]}, "strong is structure, not output syntax");
    for sample in ({{'text_html, "<p>A <strong>brass</strong> key.</p>"},
        {'text_djot, "A {*brass*} key\\."}, {'text_plain, "A brass key."}})
      $test_utils:assert_eq($document:render(document, sample[1])['body], sample[2],
        "authored prose renders as " + tostr(sample[1]));
    endfor
    const cases = {
      {"_soft_ {-old-}", "soft old"},
      {"{_ spaced _} {* bold *}", " spaced   bold "},
      {"*outer _inner_*", "outer inner"},
      {"**nested**", "nested"},
      {"_first *second_ tail*", "first *second tail*"},
      {"\\*literal\\*", "*literal*"},
      {"\\— stays literal", "\\— stays literal"},
      {"*unmatched < ordinary > punctuation", "*unmatched < ordinary > punctuation"},
      {"\"Hello,\" she said. Don't.", "“Hello,” she said. Don’t."},
      {"'twas the 'season'.", "’twas the ‘season’."},
      {"one--two---three----four...", "one–two—three––four…"},
      {"one\ntwo\\\nthree", "one two\nthree"},
      {"`` `key` ``", "`key`"},
      {"`a * b`", "a * b"},
      {"`unclosed", "unclosed"}};
    for sample in (cases)
      const value = $document:mk({$document_node:mk('paragraph, $format.djot:inline(sample[1]))});
      $test_utils:assert_eq($document:render(value, 'text_plain)['body], sample[2], sample[1]);
    endfor
    return true;
  endmethod

  method test_djot_block_structure owner: HACKER
    "Headings, paragraphs, quotes, fences, nested lists, tasks, and definitions retain structure.";
    const blocks = this:_parse("## Heading\n## continued\n\nFirst.\n\nSecond.\n\n> Quoted.\n>\n> - Item\n\n---\n\n```moo\nreturn `x';\n```\n");
    $test_utils:assert_eq({block["kind"] for block in (blocks)},
      {"heading", "paragraph", "paragraph", "block_quote", "thematic_break", "code_block"},
      "block boundaries survive parsing");
    $test_utils:assert_eq(blocks[1]["level"], 2, "heading level");
    $test_utils:assert_eq(blocks[$]["text"], "return `x';\n", "code preserves its final newline");
    $test_utils:assert_eq(blocks[$]["language"], "moo", "code language");
    const list = this:_parse("3) First\n8) Second\n\n   - [x] Finished\n   - [ ] Pending")[1];
    $test_utils:assert_eq(list["style"], "numbered", "decimal list style");
    $test_utils:assert_eq(list["start"], 3, "first ordinal defines the start");
    $test_utils:assert_eq(length(list["items"]), 2, "two outer items");
    const tasks = list["items"][2]["blocks"][2]["items"];
    $test_utils:assert_eq(tasks[1]["checked"], true, "checked task");
    $test_utils:assert_eq(tasks[2]["checked"], false, "unchecked task");
    const definition = this:_parse(": bell\n\n  A *brass* instrument.\n\n: key\n\n  Opens a door.")[1];
    $test_utils:assert_eq(definition["kind"], "definition_list", "definition list kind");
    $test_utils:assert_eq(length(definition["items"]), 2, "two terms");
    $test_utils:assert_eq(definition["items"][1]["definition"][1]["children"][2]["kind"],
      "strong", "definitions preserve inline structure");
    $test_utils:assert_eq(length(this:_parse("Prose\n- still prose")), 1,
      "a list marker cannot interrupt a paragraph");
    return true;
  endmethod

  method test_djot_links_images_and_tables owner: HACKER
    "Resolve destinations before rendering, keeping tables, image alternatives, and labels inert.";
    const blocks = this:_parse("[A *link*][site] ![a bell](https://example.org/bell.png) <a@example.org>\n\n[site]: https://example.org/help");
    const inlines = blocks[1]["children"];
    $test_utils:assert_eq(inlines[1]["url"], "https://example.org/help", "forward link definition");
    $test_utils:assert_eq(inlines[1]["children"][2]["kind"], "strong", "structured link label");
    $test_utils:assert_eq(inlines[3]["alt"], "a bell", "image alternative");
    $test_utils:assert_eq(inlines[5]["url"], "mailto:a@example.org", "email autolink");
    const distinct = this:_parse("[a][key] [b][Key]\n\n[key]: https://example.org/a\n\n[Key]: https://example.org/b")[1]["children"];
    $test_utils:assert_eq(distinct[1]["url"], "https://example.org/a", "lowercase reference label");
    $test_utils:assert_eq(distinct[3]["url"], "https://example.org/b", "uppercase reference label");
    const table = this:_parse("| Name | Value |\n|:-----|------:|\n| brass \\| `|` | *two* |\n\n^ A _caption_\n  continued")[1];
    $test_utils:assert_eq(table["columns"], {["align" -> "start"], ["align" -> "end"]},
      "table column alignment");
    $test_utils:assert_eq(length(table["header"]), 2, "table header cells");
    $test_utils:assert_eq(table["caption"][2]["kind"], "emphasis", "caption inline structure");
    $test_utils:assert_eq(length(table["rows"][1]["cells"]), 2,
      "escaped and code pipes do not split cells");
    return true;
  endmethod

  method _check_help_provider owner: ARCH_WIZARD
    "Parse and resolve each article; separate provider tests keep within the runner timeout.";
    const {provider, expected_count} = args;
    const topics = provider:help_topics(player);
    $test_utils:assert_eq(length(topics), expected_count, "All articles in the provider are covered");
    for topic in (topics)
      $test_utils:assert_eq(topic.content_kind, 'djot_blocks, topic.name + " explicitly selects Djot");
      const blocks = $format.djot:blocks(topic.content);
      const content = $help_utils:annotate_prose(blocks, player);
      const document = $event:mk_info(player, content):resolve(player);
      $test_utils:assert_true(length($document:render(document, 'text_html)['body]), topic.name);
      "Each article is an independent read-only check with its own runtime task quota.";
      suspend(0);
    endfor
    return true;
  endmethod

  method test_shipped_general_help owner: ARCH_WIZARD
    "Check every general help article.";
    return this:_check_help_provider(this.providers[1], 32);
  endmethod

  method test_shipped_programmer_help owner: ARCH_WIZARD
    "Check every programmer help article.";
    return this:_check_help_provider(this.providers[2], 8);
  endmethod

  method test_shipped_builder_help owner: ARCH_WIZARD
    "Check every builder help article.";
    return this:_check_help_provider(this.providers[3], 6);
  endmethod

  method test_shipped_admin_help owner: ARCH_WIZARD
    "Check every administrator help article.";
    return this:_check_help_provider(this.providers[4], 10);
  endmethod

  method test_shipped_wizard_help owner: ARCH_WIZARD
    "Check every wizard help article.";
    return this:_check_help_provider(this.providers[5], 13);
  endmethod

  method test_authored_producers_and_edit_validation owner: ARCH_WIZARD
    "Speech parses only the message; descriptions and notes validate selected Djot before saving.";
    const actor = create($player, player, 2);
    const note = create($note, player, 2);
    try
      actor.name = "*literal name*";
      const speech = actor:mk_say_event("Hello, *world*!"):resolve(player);
      const html = $document:render(speech, 'text_html)['body];
      $test_utils:assert_true(index(html, "*literal name*"), "actor names cannot introduce markup");
      $test_utils:assert_true(index(html, "<strong>world</strong>"), "speech markup is explicit");
      actor:set_description("A *brass* bell.\n\nThe floor is dusty.", 'djot_blocks);
      const described = $event:mk_info(player, actor:description()):resolve(player);
      $test_utils:assert_eq(length(flycontents(described)), 2, "description preserves paragraphs");
      note:set_text({"A *private* note."}, 'djot_blocks);
      for target in ({actor, note})
        let denied = false;
        try
          if (target == actor)
            target:set_description("[unsafe](javascript:bad)");
          else
            target:set_text({"[unsafe](javascript:bad)"});
          endif
        except (E_INVARG)
          denied = true;
        endtry
        $test_utils:assert_true(denied, "invalid edits fail before mutation");
      endfor
      $test_utils:assert_eq(actor.description, "A *brass* bell.\n\nThe floor is dusty.",
        "rejected description edit preserves authored source");
      $test_utils:assert_eq(note.text, {"A *private* note."}, "rejected note edit preserves text");
      actor:set_description("{n} *literal*", 'literal);
      $test_utils:assert_eq(actor:description(), "{n} *literal*", "literal input never guesses syntax");
    finally
      recycle(note);
      recycle(actor);
    endtry
    return true;
  endmethod

  method test_help_annotation_boundaries owner: ARCH_WIZARD
    "Code blocks and link labels stay opaque; unrepresentable examples remain inline code.";
    const source = "[`look`](https://example.org/) and `no_such_command`.\n\n```\nlook\n```\n\n`@chown <object>.<property> to <owner>`";
    const content = $help_utils:annotate_prose($format.djot:blocks(source), player);
    const document = $event:mk_info(player, content):resolve(player);
    const html = $document:render(document, 'text_html);
    $test_utils:assert_eq(html['annotations], [], "opaque and unsupported examples have no interactions");
    $test_utils:assert_true(index(html['body], "<code>look</code>"), "link keeps its code label");
    $test_utils:assert_true(index(html['body], "<pre><code>look"), "code block remains code");
    const example = $help_utils:annotate_prose($format.djot:blocks("Use `look`."), player);
    const rendered = $document:render($event:mk_info(player, example):resolve(player), 'text_html);
    $test_utils:assert_true(index(rendered['body], "<code>look</code>"),
      "eligible command keeps the original inline-code label");
    return true;
  endmethod

  method test_djot_diagnostics_and_limits owner: HACKER
    "Unsupported constructs, bad destinations, block input, and resource excess fail explicitly.";
    for source in ({"{+new+}", "{+two\nlines+}", "{=bright=}", "x~2~", "x^2^", "$`math`", "[^note]",
        "[span]{.class}", "`raw`{=html}", "::: box\ntext\n::: ", "a) alphabetic",
        "```=html\n<b>raw</b>\n```", "[lost][missing]", "[local](#section)",
        "[unsafe](javascript:alert(1))", "{#anchor}", "####### too deep",
        "[label][Case]\n\n[case]: https://example.org"})
      let message = "";
      try
        $format.djot:blocks(source);
      except error (E_INVARG)
        message = error[2];
      endtry
      $test_utils:assert_true(index(message, "Djot line 1, column "), source);
    endfor
    for source in ({"# title", "- item", "a\n\nb", "> quote", "```\ncode\n```"})
      let denied = false;
      try
        $format.djot:inline(source);
      except (E_INVARG)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "inline input rejects " + source);
    endfor
    let oversized = false;
    try
      $format.djot:blocks($str_proto:space(32769, "é"));
    except (E_QUOTA)
      oversized = true;
    endtry
    $test_utils:assert_true(oversized, "source limit counts UTF-8 bytes");
    const stars = $str_proto:space(40, "*");
    for source in ({$str_proto:space(80, "> ") + "nested",
        stars + "nested" + stars,
        "![" + stars + "nested" + stars + "](https://example.org/a.png)"})
      let deep = false;
      try
        $format.djot:blocks(source);
      except (E_QUOTA)
        deep = true;
      endtry
      $test_utils:assert_true(deep, "nesting is bounded even in flattened image alternatives");
    endfor
    return true;
  endmethod
endobject
