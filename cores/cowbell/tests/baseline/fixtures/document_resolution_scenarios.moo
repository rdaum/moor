object DOCUMENT_RESOLUTION_SCENARIOS
  name: "Document Resolution Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property calls (owner: ARCH_WIZARD, flags: "r") = 0;
  property secret (owner: ARCH_WIZARD, flags: "") = "private fixture content";

  method resolve_inline owner: ARCH_WIZARD
    "Fixture hooks expose call count, authority, invalid results, and recursive expansion.";
    const {context} = args;
    const principal = caller_perms();
    if (this.mode == 'private)
      set_task_perms(principal);
      return $format:literal_inline(this.secret);
    elseif (this.mode == 'authority)
      return $format:literal_inline(tostr(context['principal]) + ":" + tostr(principal));
    elseif (this.mode == 'recursive)
      return {$format:producer_inline(this)};
    elseif (this.mode == 'invalid)
      return {"unstructured callback result"};
    elseif (this.mode == 'fail)
      raise(E_INVARG, "Unselected branch must not run");
    endif
    #90122.calls = #90122.calls + 1;
    return {$format.annotation:command("look", "Look"), $format.text:mk(" *literal*")};
  endmethod

  method resolve_blocks owner: ARCH_WIZARD
    "A block producer returns block templates that contain nested inline producers.";
    const {context} = args;
    return {$format.title:mk("Report", 2),
      $format.paragraph:mk($format:producer_inline(<#90122, .mode = 'count>))};
  endmethod

  method _resolve_as owner: ARCH_WIZARD
    "Invoke resolution with a real reduced principal, without accepting a context map.";
    const {event, principal, viewer} = args;
    set_task_perms(principal);
    return event:resolve(viewer);
  endmethod

  method _plain owner: ARCH_WIZARD
    "Read the plain serialization of a resolved document.";
    return $document:render(args[1], 'text_plain)['body];
  endmethod

  method test_literal_constructors owner: ARCH_WIZARD
    "Authoring splits literal paragraphs and hard breaks, while interpolation never becomes markup.";
    const blocks = $format:literal_blocks("First *literal*.\ncontinued\n\nSecond.");
    const event = $event:mk_info(#90102, $document_template:mk('blocks, blocks));
    const document = event:resolve(#90102);
    const encoded = $document:encode(document);
    $test_utils:assert_eq(length(encoded["blocks"]), 2, "Blank lines separate paragraphs");
    $test_utils:assert_eq(encoded["blocks"][1]["children"][2]["kind"], "hard_break",
      "Other newlines are explicit hard breaks");
    const bound = $event:mk_info(#90102, $sub:binding('message))
      :with_text_binding('message, "*not emphasis* <tag>");
    const html = $document:render(bound:resolve(#90102), 'text_html)['body];
    $test_utils:assert_eq(html, "<p>*not emphasis* &lt;tag&gt;</p>", "Literal text binding");
    return true;
  endmethod

  method test_perspective_and_captured_location owner: ARCH_WIZARD
    "Movement after creation cannot change the action location; resolved labels no longer read objects.";
    const room = create($room);
    const elsewhere = create($room);
    const actor = create($thing);
    const item = create($thing);
    try
      room.name = "old room";
      elsewhere.name = "new room";
      actor.name = "alice";
      item.name = "brass *key*";
      move(actor, room);
      const event = $event:mk_take(actor, $sub:nc(), " ", $sub:self_alt("take", "takes"),
        " ", $sub:the('d), " in ", $sub:l(), "."):with_dobj(item);
      move(actor, elsewhere);
      const own = event:resolve(actor);
      const observed = event:resolve(#90102);
      $test_utils:assert_eq(this:_plain(own), "You take the brass *key* in old room.", "Actor view");
      $test_utils:assert_eq(this:_plain(observed), "Alice takes the brass *key* in old room.", "Observer view");
      const passage = $passage:mk(room, "east", {}, "", false,
        elsewhere, "west", {}, "", false);
      const template = $sub_utils:compile("{nc} {step|steps} {direction} from {from_room} to {to_room}.");
      const departure = passage:movement_event(actor, room, elsewhere, "east", "west", template);
      const arrival = passage:movement_event(actor, room, elsewhere, "east", "west", template, true);
      $test_utils:assert_eq(this:_plain(departure:resolve(actor)),
        "You step east from old room to new room.", "Custom departure preserves actor perspective");
      $test_utils:assert_eq(this:_plain(departure:resolve(#90102)),
        "Alice steps east from old room to new room.", "Custom departure preserves observer perspective");
      $test_utils:assert_eq(this:_plain(arrival:resolve(#90102)),
        "Alice steps west from old room to new room.", "Arrival uses the destination direction");
      $test_utils:assert_eq({departure.location, arrival.location}, {room, elsewhere},
        "Both action locations are explicit even after the traveler moves");
      actor.name = "renamed actor";
      room.name = "renamed room";
      item.name = "renamed item";
      for format in ({'text_html, 'text_djot, 'text_plain})
        const result = $document:render(observed, format);
        $test_utils:assert_true(!index(result['body], "renamed"), "Renderers use resolved labels");
      endfor
      $test_utils:assert_eq(length($document:render(observed, 'text_html)['annotations]), 3,
        "Actor, object, and location retain semantic references");
    finally
      recycle(actor);
      recycle(item);
      recycle(room);
      recycle(elsewhere);
    endtry
    return true;
  endmethod

  method test_selected_branch_and_typed_bindings owner: ARCH_WIZARD
    "Only the selected alternative runs. Text and structured bindings have separate operations.";
    const bomb = $format:producer_inline(<#90122, .mode = 'fail>);
    const event = $event:mk_info(#90102,
      $sub:self_altc($format.paragraph:inline("safe ", $format.code:inline("branch")), bomb));
    $test_utils:assert_eq(this:_plain(event:resolve(#90102)), "Safe branch", "Selected branch only");
    const content = $document_template:mk('strong, $format:literal_inline("structured"));
    const bound = $event:mk_info(#90102, $sub:content_binding('value))
      :with_content_binding('value, content);
    $test_utils:assert_eq($document:render(bound:resolve(#90102), 'text_html)['body],
      "<p><strong>structured</strong></p>", "Explicit content binding");
    const wrong = $event:mk_info(#90102, $sub:binding('value))
      :with_content_binding('value, content);
    $test_utils:assert_raises(E_INVARG, wrong, "resolve", {#90102}, "No implicit content coercion");
    return true;
  endmethod

  method test_hooks_run_before_rendering owner: ARCH_WIZARD
    "A resolved document contains only canonical values and never calls its producer during rendering.";
    this.calls = 0;
    const event = $event:mk_info(#90102, $format:producer_blocks(this));
    const document = event:resolve(#90102);
    $test_utils:assert_eq(this.calls, 1, "One nested inline expansion");
    const encoded = $document:encode(document);
    for format in ({'text_html, 'text_djot, 'text_plain})
      const result = $document:render(document, format);
      $test_utils:assert_true(index(result['body], "Look"), "Resolved hook output");
    endfor
    $test_utils:assert_eq(this.calls, 1, "Serializers do not repeat hooks");
    $test_utils:assert_eq($document:encode($document:decode(encoded)), encoded, "No unresolved producer metadata");
    return true;
  endmethod

  method test_resolution_authority owner: ARCH_WIZARD
    "Hooks see the actual principal. A different viewer does not grant access to private content.";
    const probe = $event:mk_info(#90102,
      $format:producer_inline(<#90122, .mode = 'authority>));
    const document = this:_resolve_as(probe, #90102, #90100);
    $test_utils:assert_eq(this:_plain(document), tostr(#90102) + ":" + tostr(#90102),
      "Viewer and authority remain separate");
    const private = $event:mk_info(#90102,
      $format:producer_inline(<#90122, .mode = 'private>));
    $test_utils:assert_raises(E_PERM, this, "_resolve_as", {private, #90102, #90100},
      "Resolver does not lend its owner privileges");
    $test_utils:assert_raises(E_ARGS, probe, "resolve", {#90102, ['principal -> #90100]},
      "An ordinary map cannot set the principal");
    return true;
  endmethod

  method test_public_content_without_viewer owner: ARCH_WIZARD
    "Public pre-authentication content needs no synthetic actor or viewer.";
    const public = $event:mk_welcome(false, $format.title:mk("Welcome"), "Public text.");
    $test_utils:assert_eq(this:_plain(public:resolve(false)), "Welcome\n\nPublic text.", "Public document");
    const perspective = $event:mk_info(#90102, $sub:n());
    $test_utils:assert_raises(E_INVARG, perspective, "resolve", {false}, "Perspective requires a viewer");
    return true;
  endmethod

  method test_invalid_and_recursive_hooks owner: ARCH_WIZARD
    "Invalid hook results and recursive producers fail during resolution with an explicit path.";
    for mode in ({'invalid, 'recursive})
      const event = $event:mk_info(#90102,
        $format:producer_inline(<#90122, .mode = mode>));
      try
        event:resolve(#90102);
      except error (E_INVARG, E_QUOTA)
        $test_utils:assert_true(index(error[2], "document.blocks[0]", 1), "Expansion diagnostic path");
        continue;
      endtry
      raise(E_ASSERT, "Invalid hook unexpectedly resolved");
    endfor
    return true;
  endmethod

  method test_structured_factories owner: ARCH_WIZARD
    "Factory output preserves multi-block list items, definitions, table cells, and opaque source.";
    const cell = $format.block:mk($format.paragraph:mk("First."), $format.paragraph:mk("Second."));
    const event = $event:mk_info(#90102,
      $format.list:mk({cell}, true, 4), $format.deflist:mk({{"Term", cell}}),
      $format.table:mk({"Header"}, {{cell}}), $format.code:mk("line\n```", "moo", 21));
    const document = event:resolve(#90102);
    const blocks = $document:encode(document)["blocks"];
    $test_utils:assert_eq(length(blocks[1]["items"][1]["blocks"]), 2, "List item paragraphs");
    $test_utils:assert_eq(length(blocks[2]["items"][1]["definition"]), 2, "Definition paragraphs");
    $test_utils:assert_eq(length(blocks[3]["rows"][1]["cells"][1]["blocks"]), 2, "Table cell paragraphs");
    $test_utils:assert_eq(blocks[4]["first_line"], 21, "Separate source line origin");
    $test_utils:assert_raises(E_INVARG, $format.table, "mk", {{"A", "B"}, {}, {'start}},
      "Column alignments must cover every column");
    $test_utils:assert_raises(E_INVARG, $format.table, "mk", {{"A"}, {}, {'unknown}},
      "Only document alignments are accepted");
    let headerless = $format.table:mk({}, {{"Cell"}});
    headerless.header = false;
    const encoded = $document:encode($event:mk_info(#90102, headerless):resolve(#90102));
    $test_utils:assert_false(maphaskey(encoded["blocks"][1], "header"), "Absent headers remain absent");
    for format in ({'text_html, 'text_djot, 'text_plain})
      const result = $document:render(document, format);
      $test_utils:assert_true(index(result['body], "First"), "Complete content in every format");
    endfor
    return true;
  endmethod

  method test_random_choice_is_resolved owner: ARCH_WIZARD
    "A message bag chooses its template once before several serializers consume the document.";
    const bag = $msg_bag:mk({"One"}, {"Two"});
    const document = $event:mk_info(#90102, $format:producer_inline(bag)):resolve(#90102);
    const choice = this:_plain(document);
    $test_utils:assert_true(choice in {"One", "Two"}, "One selected message");
    for format in ({'text_html, 'text_djot})
      $test_utils:assert_true(index($document:render(document, format)['body], choice), "Same choice in all formats");
    endfor
    return true;
  endmethod
  method test_private_message_documents owner: ARCH_WIZARD
    "DM attribution retains references and body paragraphs without interpreting literal punctuation.";
    const message = $dm:mk(#90102, #90101, "First *literal*.\n\nSecond <literal>.");
    const received = message:display_event(#90101):resolve(#90101);
    const sent = message:sender_echo_event():resolve(#90102);
    for document in ({received, sent})
      const encoded = $document:encode(document);
      $test_utils:assert_eq(length(encoded["blocks"]), 3, "Attribution and both body paragraphs");
      $test_utils:assert_eq(encoded["blocks"][2]["children"][1]["text"], "First *literal*.",
        "Message punctuation remains text");
      for format in ({'text_html, 'text_djot, 'text_plain})
        const rendered = $document:render(document, format);
        $test_utils:assert_false(index(rendered['body], "\\x1b["), "No literal ANSI sequence");
        $test_utils:assert_false(index(rendered['body], binary_to_str(b"Gw==")), "No terminal escape");
        $test_utils:assert_true(index(rendered['body], "Second"), "Complete body in every format");
        $test_utils:assert_eq(length(rendered['annotations]), format == 'text_plain ? 0 | 2,
          "Participant and captured location references");
      endfor
    endfor
    $test_utils:assert_eq(message:sender_echo_event():audience(), 'utility, "Receipt stays private utility output");
    return true;
  endmethod

  method test_agent_report_documents owner: ARCH_WIZARD
    "Agent report tools emit code, tables, and literal prose through the normal room event path.";
    const room = create($agent_room, player, 2);
    try
      add_property(room, "captured_event", false, {player, "r"});
      add_verb(room, {player, "rxd", "announce"}, {"this", "none", "this"});
      $test_utils:assert_false(set_verb_code(room, "announce", {"this.captured_event = args[1];"}),
        "Room capture compiles");
      const source = "before\n```\n*opaque*";
      room:_tool_present_code(["title" -> "Source", "code" -> source, "language" -> "moo"], player);
      const code = room.captured_event:resolve(#90102);
      const blocks = $document:encode(code)["blocks"];
      $test_utils:assert_eq({blocks[1]["kind"], blocks[2]["kind"]}, {"heading", "code_block"},
        "Code presentation uses structural nodes");
      $test_utils:assert_eq(blocks[2]["text"], source, "Code delimiters stay inside the source buffer");
      room:_tool_present_table(["title" -> "Cells", "headers" -> {"Left |", "Right"},
        "rows" -> {{"First\nSecond", "<tag>"}}], player);
      const table = room.captured_event:resolve(#90102);
      const encoded = $document:encode(table)["blocks"][2];
      $test_utils:assert_eq(encoded["kind"], "table", "Table presentation uses a table node");
      $test_utils:assert_eq(length(encoded["rows"][1]["cells"]), 2, "Punctuation cannot add a column");
      $test_utils:assert_eq(encoded["rows"][1]["cells"][1]["blocks"][1]["children"][2]["kind"],
        "hard_break", "Cell newlines retain their structure");
      const last_event = room.captured_event;
      let rejected = false;
      try
        room:_tool_present_table(["headers" -> {"Only"}, "rows" -> {{"valid"}, "invalid"}], player);
      except (E_TYPE)
        rejected = true;
      endtry
      $test_utils:assert_true(rejected, "Malformed rows cannot silently disappear");
      $test_utils:assert_eq(room.captured_event, last_event, "Invalid table publishes no partial report");
      room:_tool_present_report(["title" -> "Report", "content" -> "*literal*\n\nSecond paragraph."], player);
      const report = room.captured_event:resolve(#90102);
      $test_utils:assert_eq(length($document:encode(report)["blocks"]), 3, "Report paragraphs remain separate");
      for document in ({code, table, report})
        for format in ({'text_html, 'text_djot, 'text_plain})
          $test_utils:assert_true(length($document:render(document, format)['body]), "Report renders in every format");
        endfor
      endfor
    finally
      recycle(room);
    endtry
    return true;
  endmethod

  method test_programmatic_description_blocks owner: ARCH_WIZARD
    "Both agent room descriptions retain headings and lists through the ordinary look producer.";
    for prototype in ({$agent_room, $agentic.coding_room})
      const room = create(prototype, player, 2);
      try
        const document = $look:narrative(room:look_self():capture(player)):resolve(player);
        let lists = 0;
        for block in ($document:encode(document)["blocks"])
          block["kind"] == "list" && (lists = lists + 1);
        endfor
        $test_utils:assert_true(lists > 0, "Description lists remain top-level blocks");
        for format in ({'text_html, 'text_djot, 'text_plain})
          const rendered = $document:render(document, format);
          $test_utils:assert_true(index(rendered['body], "Queue a task"), "Description commands remain visible");
        endfor
      finally
        recycle(room);
      endtry
    endfor
    return true;
  endmethod

  method test_source_listing_documents owner: ARCH_WIZARD
    "Visor ranges retain exact source and line origins; documentation display preserves explicit code blocks.";
    const visor = create($data_visor, player, 2);
    const target = create($thing);
    try
      add_verb(target, {player, "rxd", "listing_probe"}, {"this", "none", "this"});
      $test_utils:assert_false(set_verb_code(target, "listing_probe",
        {"\"Source *literal*.\";", "let value = 1;", "value = value + 1;", "return value;"}), "Source compiles");
      add_verb(visor, {player, "rxd", "listing_probe"}, {"this", "none", "this"});
      $test_utils:assert_false(set_verb_code(visor, "listing_probe",
        {"return this:_tool_build_present_verb_code_payload(@args);"}), "Presentation probe compiles");
      const source = verb_code(target, "listing_probe", false, true);
      const request = ["object" -> tostr(target), "verb" -> "listing_probe", "start_line" -> 2, "end_line" -> 3];
      const range = visor:listing_probe(request, player, true);
      const document = $event:mk_info(player, range["content"]):resolve(player);
      const code = $document:encode(document)["blocks"][2];
      $test_utils:assert_eq(code["first_line"], 2, "Range numbering uses its actual start");
      $test_utils:assert_eq(code["text"], source[2..3]:join("\n"), "Number gutters never enter source text");
      $test_utils:assert_true(index($document:render(document, 'text_plain)['body], "2 | " + source[2]),
        "Renderer applies the source origin");
      const full = visor:listing_probe(request, player, false);
      const full_code = $document:encode($event:mk_info(player, full["content"]):resolve(player))["blocks"][2];
      $test_utils:assert_eq(full_code["text"], source:join("\n"), "Whole listing preserves source");
      const content = $help_utils:format_documentation_display(
        {"Source ", $format.code:inline("listing_probe")}, $format.code:mk(source, "moo", 1));
      const documentation = $document:encode($event:mk_info(player, content):resolve(player));
      $test_utils:assert_eq(documentation["blocks"][1]["kind"], "heading", "Documentation title has structure");
      $test_utils:assert_eq(documentation["blocks"][2]["text"], source:join("\n"),
        "A flyweight code block is not mistaken for missing documentation");
    finally
      recycle(visor);
      recycle(target);
    endtry
    return true;
  endmethod

  method test_container_description_documents owner: ARCH_WIZARD
    "Container looks preserve blocks and references, and closed containers resolve denial substitutions without revealing contents.";
    const container = create($container);
    const item = create($thing);
    try
      container.name = "document box";
      container.description = $format.block:mk("First paragraph.", "Second paragraph.");
      container.view_denied_msg = $sub_utils:compile("{The i} is shut.");
      item.name = "hidden *token*";
      move(container, player);
      move(item, container);
      container.open = false;
      const closed = $look:narrative(container:look_self():capture(player)):resolve(player);
      $test_utils:assert_eq(this:_plain(closed),
        "document box\n\nFirst paragraph.\n\nSecond paragraph.\n\nThe document box is shut.",
        "Closed view retains the description and resolves its own container binding");
      container.open = true;
      const opened = $look:narrative(container:look_self():capture(player)):resolve(player);
      $test_utils:assert_true(index(this:_plain(opened), "It contains hidden *token*."), "Visible contents are literal");
      $test_utils:assert_eq(length($document:render(opened, 'text_html)['annotations]), 2,
        "Container and visible contents retain references");
    finally
      recycle(item);
      recycle(container);
    endtry
    return true;
  endmethod

  method test_generated_help_document owner: ARCH_WIZARD
    "A HINT-generated help topic carries its command as a reference without reparsing markup.";
    const topic = $help_utils:verb_help_from_hint($prog_features, "@doc");
    const event = $event:mk_info(player, $format.block:mk(@topic:render_prose()));
    const document = event:resolve(player);
    const rendered = $document:render(document, 'text_html);
    $test_utils:assert_true(index(rendered['body], "<code>@doc &lt;object|builtin&gt;</code>"),
      "Usage retains its code label");
    $test_utils:assert_true(length(rendered['annotations]) > 0, "Usage retains its command reference");
    $test_utils:assert_eq(this:_plain(document),
      "Display developer documentation.\n\nUsage: @doc <object|builtin>", "Plain usage has no backtick delimiters");
    return true;
  endmethod
endobject
