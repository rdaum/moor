object DELIVERY_SCENARIOS
  name: "Document Delivery Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property encodes (owner: ARCH_WIZARD, flags: "rw") = 0;
  property renders (owner: ARCH_WIZARD, flags: "rw") = 0;

  method test_offline_room_snapshot_skips_content owner: ARCH_WIZARD
    "A live room panel has no recipient when the player has no attached connections.";
    const receiver = #90102;
    $test_utils:assert_eq(`connections(receiver) ! E_INVARG => {}', {}, "fixture player is disconnected");
    add_verb(receiver, {$arch_wizard, "rxd", "_capture_room"}, {"this", "none", "this"});
    set_verb_code(receiver, "_capture_room", {"raise(E_ASSERT, \"Disconnected panel invoked room content\");"});
    try
      $test_utils:assert_false(receiver:emit_room_snapshot_state(receiver, receiver.location),
        "An offline player has no live panel to refresh");
      $test_utils:assert_false(receiver:emit_room_look(receiver.location),
        "An offline arrival has no presentation recipient");
    finally
      delete_verb(receiver, "_capture_room");
    endtry
    return true;
  endmethod

  method _instrument owner: ARCH_WIZARD
    "Count actual encoder and renderer entry calls inside this isolated test database.";
    const {target, method_name, counter} = args;
    const original = verb_code(target, method_name);
    const increment = "#90125." + counter + " = #90125." + counter + " + 1;";
    $test_utils:assert_eq(set_verb_code(target, method_name, {increment, @original}), 0,
      "counter wrapper compiles");
    return original;
  endmethod

  method _receiver owner: ARCH_WIZARD
    "Capture effects without network connections; ordinary delivery code still resolves and serializes.";
    const receiver = create($event_receiver, player, 2);
    add_property(receiver, "test_notices", {}, {player, "r"});
    add_property(receiver, "test_history", {}, {player, "r"});
    add_property(receiver, "test_connections", {
      {#-1, "first", 0, {"text/html"}}, {#-2, "second", 0, {"text/html"}},
      {#-3, "third", 0, {"text/djot"}}}, {player, "r"});
    for definition in ({
        {"_connections", "return this.test_connections;"},
        {"_notify", "this.test_notices = {@this.test_notices, args};"},
        {"_event_log", "this.test_history = {@this.test_history, args};"},
        {"_connection_entry", "return this.test_connections[1];"}})
      const {method_name, code} = definition;
      add_verb(receiver, {player, "rxd", method_name}, {"this", "none", "this"});
      $test_utils:assert_eq(set_verb_code(receiver, method_name, {code}), 0, "effect capture compiles");
    endfor
    return receiver;
  endmethod

  method test_shared_delivery_document owner: ARCH_WIZARD
    "History and mixed connections share one encoding; unused bindings and stale envelopes stay private.";
    const receiver = this:_receiver();
    const encode_code = this:_instrument($document_schema, "_checked", "encodes");
    const render_code = this:_instrument($document_renderer, "_render", "renders");
    try
      this.encodes = this.renders = #90122.calls = 0;
      const event = $event:mk_info(#90102, $format:producer_inline(<#90122, .mode = 'count>))
        :with_text_binding('secret, "unused private binding")
        :with_metadata('cowbell_document, ["stale" -> true])
        :with_metadata('annotations, ["stale" -> true]);
      receiver:tell(event);
      $test_utils:assert_eq(this.encodes, 1, "one encoding for the entire delivery");
      $test_utils:assert_eq(this.renders, 2, "HTML and history Djot each render once");
      $test_utils:assert_eq(#90122.calls, 1, "producer resolution happens once");
      $test_utils:assert_eq(length(receiver.test_history), 1, "one history record");
      $test_utils:assert_eq(length(receiver.test_notices), 3, "one notice per connection");
      const history = receiver.test_history[1];
      const metadata = history[4];
      $test_utils:assert_false(maphaskey(metadata, 'bindings), "unselected bindings never enter metadata");
      $test_utils:assert_eq(metadata['cowbell_document]["version"], 1, "stale document is replaced");
      $test_utils:assert_false(maphaskey(metadata['annotations], "stale"), "stale annotations are replaced");
      for notice in (receiver.test_notices)
        $test_utils:assert_eq(notice[6]['cowbell_document], metadata['cowbell_document],
          "every format carries the same captured document");
        $test_utils:assert_eq(notice[6]['delivery_id], metadata['delivery_id], "one delivery identity");
        $test_utils:assert_false(maphaskey(notice[6], 'bindings), "live output excludes bindings");
      endfor
      $test_utils:assert_eq(receiver.test_notices[1][2], receiver.test_notices[2][2],
        "equal connection formats reuse the body");
      $test_utils:assert_eq(receiver.test_notices[3][2], history[2], "history rendering serves live Djot");
      $test_utils:assert_eq(receiver.test_notices[3][6]['annotations], metadata['annotations],
        "history rendering serves the same body annotations");
      receiver.test_notices = {};
      receiver.test_history = {};
      receiver:inform_all($event:mk_info(receiver, "transient"));
      $test_utils:assert_eq(length(receiver.test_notices), 3, "utility fanout retains all connections");
      $test_utils:assert_eq(receiver.test_history, {}, "utility fanout never creates history");
      receiver.test_notices = {};
      receiver:rewrite_event("existing", $event:mk_info(receiver, "replacement"), #-1);
      $test_utils:assert_eq(length(receiver.test_notices), 1, "rewrite stays on its captured connection");
      $test_utils:assert_eq(receiver.test_notices[1][6]['rewrite_target], "existing", "rewrite identity");
      $test_utils:assert_true(maphaskey(receiver.test_notices[1][6], 'cowbell_document), "rewrite carries document");
      $test_utils:assert_eq(receiver.test_history, {}, "utility rewrite never creates history");
      receiver.test_connections = {};
      receiver.test_notices = {};
      #90122.calls = 0;
      receiver:inform_all($event:mk_info(receiver, $format:producer_inline(<#90122, .mode = 'count>)));
      $test_utils:assert_eq(#90122.calls, 0, "offline utility output does not resolve content");
      $test_utils:assert_eq(receiver.test_notices, {}, "offline utility output has no destinations");
      $test_utils:assert_eq(receiver.test_history, {}, "offline utility output creates no history");
    finally
      set_verb_code($document_schema, "_checked", encode_code);
      set_verb_code($document_renderer, "_render", render_code);
      recycle(receiver);
    endtry
    return true;
  endmethod

  method test_render_options_and_finalization owner: ARCH_WIZARD
    "Canonical option sets share a rendering, while changed options retain the same document.";
    const render_code = this:_instrument($document_renderer, "_render", "renders");
    try
      this.renders = 0;
      const code = $document_node:mk('code_block, {}, ['text -> "return 1;", 'first_line -> 10]);
      const result = $document:render_many($document:mk({code}), {
        ['format -> 'text_plain],
        ['format -> 'text_plain, 'options -> ['width -> 80, 'code_line_numbers -> true]],
        ['format -> 'text_plain, 'options -> ['code_line_numbers -> false]]});
      $test_utils:assert_eq(this.renders, 2, "only distinct normalized options render");
      $test_utils:assert_eq(result['results][1], result['results][2], "equivalent defaults share output");
      $test_utils:assert_true(result['results][1]['body] != result['results][3]['body],
        "line number selection changes the serialized body");
      const invalid = $document_node:mk('reference, $format:literal_inline("still readable"),
        ['descriptor -> ['kind -> 'object, 'ref -> "invalid"]]);
      const paragraph = $document_node:mk('paragraph, {invalid});
      const finalized = $document:render_many($document:mk({paragraph}),
        {['format -> 'text_html], ['format -> 'text_djot]});
      $test_utils:assert_eq(length(finalized['diagnostics]), 1, "bad interaction gets one diagnostic");
      $test_utils:assert_eq(finalized['document]["blocks"][1]["children"][1]["kind"], "text",
        "label-only fallback is applied before metadata is shared");
      for representation in (finalized['results])
        $test_utils:assert_eq(representation['annotations], [], "all bodies follow finalized references");
      endfor
    finally
      set_verb_code($document_renderer, "_render", render_code);
    endtry
    return true;
  endmethod

  method test_public_welcome_and_manifest owner: ARCH_WIZARD
    "Banner facts stay literal after Djot parsing; the managed package owns the document pipeline.";
    const message = $login.welcome_message;
    const title = $login.moo_title;
    try
      $login.moo_title = "*untrusted* <name> {VERSION}";
      $login.welcome_message = {"# Welcome {TITLE}", "", "Version {VERSION}", "", "`{TITLE}`"};
      const event = $event:mk_info(false, $login:welcome_content());
      const batch = event:render_for(false, {['format -> 'text_html]});
      const body = batch['results][1]['body];
      $test_utils:assert_true(index(body, "*untrusted* &lt;name&gt; {VERSION}") > 0,
        "inserted facts are neither markup nor further substitutions");
      $test_utils:assert_true(index(body, "<code>{TITLE}</code>") > 0, "code remains opaque");
      $test_utils:assert_true(index(body, "Version " + server_version()) > 0, "source placeholders expand");
      $test_utils:assert_raises(E_PERM, $login, "_public_output", {"forged"}, "login output boundary is private");
    finally
      $login.welcome_message = message;
      $login.moo_title = title;
    endtry
    const package = $change_manager.packages["cowbell"];
    for component in ({$document, $document_node, $document_template, $document_resolver,
        $document_renderer, $document_schema, $format.djot})
      $test_utils:assert_true(component in package["objects"] != 0, "document component is managed");
    endfor
    $test_utils:assert_false(maphaskey(package["constants"], "ANSI"), "obsolete terminal API removed");
    $test_utils:assert_false(maphaskey(package["constants"], "HTML"), "obsolete markup API removed");
    return true;
  endmethod
  method test_dynamic_authored_help owner: ARCH_WIZARD
    "Contextual topics select Djot for authored markup and preserve their block structure.";
    for provider in ({$room, $social_features, $mail_features, $mr_welcome})
      for topic in (provider:help_topics(player))
        if (typeof(topic.content) == TYPE_STR && index(topic.content, "`"))
          $test_utils:assert_eq(topic.content_kind, 'djot_blocks, topic.name);
          const blocks = $format.djot:blocks(topic.content);
          const content = $help_utils:annotate_prose(blocks, player);
          const batch = $event:mk_info(player, content):render_for(player, {['format -> 'text_html]});
          $test_utils:assert_true(length(batch['results][1]['body]), topic.name);
          if (topic.name == "doors")
            $test_utils:assert_true(index(batch['results][1]['body], "<ul>") > 0,
              "door choices remain a list");
          endif
          suspend(0);
        endif
      endfor
    endfor
    return true;
  endmethod
endobject
