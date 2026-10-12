// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_EVENT_SCENARIOS
  name: "Headless Event Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for pure event and substitution rendering.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_event_scenarios";

  method _descriptor owner: ARCH_WIZARD
    "Read a reference through its public document and render contracts.";
    const {reference} = args;
    const result = $document:render($event:mk_info(player, reference):resolve(player), 'text_djot);
    return mapvalues(result['annotations])[1];
  endmethod

  verb _fixtures (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Create actor, observer, room, and item fixtures for rendering scenarios.";
    room = create($room);
    actor = create($thing);
    observer = create($thing);
    item = create($thing);
    room:set_name_aliases("headless event room", {"headless-event-room"});
    actor:set_name_aliases("headless event actor", {"headless-event-actor"});
    observer:set_name_aliases("headless event observer", {"headless-event-observer"});
    item:set_name_aliases("headless event token", {"headless-event-token"});
    actor:moveto(room);
    observer:moveto(room);
    item:moveto(room);
    return {actor, observer, room, item};
  endverb

  verb _destroy_fixtures (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Destroy valid persistent fixtures in reverse containment order.";
    {actor, observer, room, item} = args;
    valid(item) && item:destroy();
    valid(observer) && observer:destroy();
    valid(actor) && actor:destroy();
    valid(room) && room:destroy();
    return true;
  endverb

  verb test_headless_event_perspective_rendering (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: event substitution renders actor and observer perspectives without delivery.";
    actor = observer = room = item = #-1;
    try
      {actor, observer, room, item} = this:_fixtures();
      event = $event:mk_info(actor, $sub:nc(), " ", $sub:self_alt("take", "takes"), " ", $sub:the('d), " in ", $sub:l(), "."):with_dobj(item);
      $test_utils:assert_true(event:validate(), "event should validate before rendering");
      $test_utils:assert_eq($document:render(event:resolve(actor), 'text_plain)['body], "You take the headless event token in headless event room.", "actor render should use second-person substitution");
      $test_utils:assert_eq($document:render(event:resolve(observer), 'text_plain)['body], "Headless event actor takes the headless event token in headless event room.", "observer render should use third-person substitution");
    finally
      this:_destroy_fixtures(actor, observer, room, item);
    endtry
    return true;
  endverb

  verb test_headless_event_message_bag_template_rendering (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: message bag entries can be compiled, picked, and rendered through events.";
    actor = observer = room = item = #-1;
    try
      {actor, observer, room, item} = this:_fixtures();
      template = "{nc} {have} {the d}.";
      compiled = $sub_utils:compile(template);
      bag = $msg_bag:mk(compiled);
      $test_utils:assert_true($msg_bag:is_msg_bag(bag), "compiled flyweight should be a message bag");
      $test_utils:assert_eq($sub_utils:decompile(compiled), template, "compiled template should decompile");
      picked = bag:pick();
      event = $event:mk_info(actor, @picked):with_dobj(item);
      $test_utils:assert_eq($document:render(event:resolve(actor), 'text_plain)['body], "You have the headless event token.", "actor render should use message bag template");
      $test_utils:assert_eq($document:render(event:resolve(observer), 'text_plain)['body], "Headless event actor has the headless event token.", "observer render should conjugate message bag template");
    finally
      this:_destroy_fixtures(actor, observer, room, item);
    endtry
    return true;
  endverb

  verb test_headless_event_message_bag_mutation (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: message bag flyweight mutation preserves compiled template rendering.";
    actor = observer = room = item = #-1;
    try
      {actor, observer, room, item} = this:_fixtures();
      bag = $msg_bag:mk($sub_utils:compile("{nc} {look} at {the d}."));
      bag = bag:add($sub_utils:compile("{the dc} {be_dobj} here."));
      $test_utils:assert_eq(length(bag:entries()), 2, "add should append a message bag entry");
      bag = bag:set_entry(2, $sub_utils:compile("{nc} {feel|feels} ready."));
      bag = bag:remove(1);
      $test_utils:assert_eq(length(bag:entries()), 1, "remove should delete a message bag entry");
      picked = bag:pick();
      event = $event:mk_info(actor, @picked):with_dobj(item);
      $test_utils:assert_eq($document:render(event:resolve(actor), 'text_plain)['body], "You feel ready.", "mutated message bag should render actor perspective");
      $test_utils:assert_eq($document:render(event:resolve(observer), 'text_plain)['body], "Headless event actor feels ready.", "mutated message bag should render observer perspective");
    finally
      this:_destroy_fixtures(actor, observer, room, item);
    endtry
    return true;
  endverb
  verb test_semantic_annotation_composition (this none this) owner: ARCH_WIZARD flags: "rxd"
    const label = "A [brass] key <&> | *bright*";
    const object = $format.annotation:object(player, label);
    const paragraph = $format.paragraph:mk({object, " and ", object});
    const event = $event:mk_info(player, $format.block:mk($format.title:mk(object), paragraph));
    const plain = $document:render(event:resolve(player), 'text_plain);
    $test_utils:assert_eq(plain['annotations], [], "telnet has no annotation metadata");
    $test_utils:assert_true(index(plain['body], label), "telnet retains authored labels");
    const djot = $document:render(event:resolve(player), 'text_djot);
    const html = $document:render(event:resolve(player), 'text_html);
    $test_utils:assert_eq(length(djot['annotations]), 3, "each occurrence has an anchor");
    $test_utils:assert_eq(length(html['annotations]), 3, "HTML preserves nested annotations");
    const source = djot['body];
    $test_utils:assert_true(index(source, "\\[brass\\]"), "Djot label punctuation is escaped");
    const rendered = html['body];
    $test_utils:assert_true(index(rendered, "&lt;&amp;&gt;"), "HTML labels are escaped");
    for id in (mapkeys(djot['annotations]))
      const descriptor = djot['annotations][id];
      $test_utils:assert_true(index(source, "annotation=" + id), "table entries refer to emitted spans");
      $test_utils:assert_eq(descriptor["ref"], $url_utils:to_curie_str(player), "reference retains identity");
      $test_utils:assert_eq(html['annotations][id], descriptor, "reference identities are stable across formats");
    endfor
    return true;
  endverb

  verb test_semantic_annotation_cells (this none this) owner: ARCH_WIZARD flags: "rxd"
    const object = $format.annotation:object(player, "player");
    const table = $format.table:mk({object}, {{object}});
    const definitions = $format.deflist:mk({{object, object}});
    const list = $format.list:mk({object});
    const event = $event:mk_info(player, table, definitions, list);
    for format in ({'text_html, 'text_djot, 'text_plain})
      const result = $document:render(event:resolve(player), format);
      $test_utils:assert_eq(length(result['annotations]), format == 'text_plain ? 0 | 5, "nested cells preserve annotations");
    endfor
    return true;
  endverb
  verb test_annotation_delivery_bundle (this none this) owner: ARCH_WIZARD flags: "rxd"
    const receiver = create($event_receiver, player, 2);
    try
      add_verb(receiver, {player, "rxd", "render_test"}, {"this", "none", "this"});
      set_verb_code(receiver, "render_test", {"return this:_event_render({{#-1, \"test\", 0, {tosym(\"text/html\")}}, {#-2, \"test\", 0, {\"text/plain\"}}, {#-3, \"test\", 0, {'text_html}}}, args[1]);"});
      #90122.calls = 0;
      const event = $event:mk_info(player, $sub:nc(), " picked up ", $sub:d(), ". ",
        $format:producer_inline(<#90122, .mode = 'count>)):with_dobj(receiver);
      const batch = receiver:render_test(event);
      const outputs = batch['connections];
      $test_utils:assert_eq(batch['document]["version"], 1, "all connections share the document envelope");
      $test_utils:assert_eq(length(outputs), 3, "all attached connections receive their negotiated format");
      $test_utils:assert_eq(typeof(outputs[1][3]), TYPE_STR, "HTML is one delivery string");
      $test_utils:assert_eq(length(outputs[1][4]), 3, "HTML owns its annotation table");
      $test_utils:assert_eq(outputs[2][4], [], "plain connection has no rich metadata");
      $test_utils:assert_eq(outputs[1][3..4], outputs[3][3..4], "same-format connections share one serialization");
      $test_utils:assert_eq(#90122.calls, 1, "connections and formats do not repeat resolution");
      $test_utils:assert_true(!index(outputs[2][3], "annotation="), "telnet contains labels only");
    finally
      recycle(receiver);
    endtry
    return true;
  endverb
  verb test_help_verb_annotations (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Help resolves real verbs, preserves code examples, and has plain text fallback.";
    const prose = $help_utils:annotate_prose($format.djot:blocks("Use `look` or `say <message>`. Keep `no_such_verb_123` and `` `look` `` intact."), $test_player);
    const event = $event:mk_info(player, prose);
    const rich = $document:render(event:resolve(player), 'text_djot);
    $test_utils:assert_eq(length(rich['annotations]), 4, "known mentions have actions and separate programmer source references");
    for id in (mapkeys(rich['annotations]))
      const ref = rich['annotations][id];
      $test_utils:assert_true(ref["kind"] in {"command", "verb"}, "help distinguishes invocation from browsing");
      if (ref["kind"] == "verb")
        $test_utils:assert_true(maphaskey(ref, "definer"), "inherited verb includes its definer");
      endif
    endfor
    const speech = this:_descriptor($help_utils:command_usage("say <message>", player, {"any", "any", "any"}));
    const shortcut = this:_descriptor($help_utils:command_usage("@changes diff", player, {"any", "any", "any"}));
    $test_utils:assert_eq(shortcut, ["kind" -> "command", "command" -> "@changes diff"], "complete commands need no argument fields");
    const look = this:_descriptor($help_utils:command_usage("look", player, {"none", "none", "none"}));
    $test_utils:assert_eq(look, ["kind" -> "command", "command" -> "look"], "argument-free verbs remain executable help links");
    $test_utils:assert_eq(speech["arguments"]["dobj"]["expectedKind"], "text", "unconstrained speech takes free text");
    $test_utils:assert_false(maphaskey(speech["arguments"]["dobj"], "suggestions"), "speech does not suggest objects");
    const plain = $document:render(event:resolve(player), 'text_plain);
    $test_utils:assert_eq(plain['annotations], [], "telnet needs no semantic metadata");
    $test_utils:assert_true(index(plain['body], "say <message>"), "telnet retains usage label");
    const list = $format.list:mk({$format.annotation:help($help_topics, "look"), "inventory"}, false);
    const list_event = $event:mk_info(player, list);
    const djot = $document:render(list_event:resolve(player), 'text_djot);
    $test_utils:assert_eq(length(djot['annotations]), 1, "topic lists retain help references");
    $test_utils:assert_eq($document:render(list_event:resolve(player), 'text_plain)['body], "- look\n\n- inventory", "plain lists stay readable");
    const topics = {$format.annotation:help($help_topics, "look"),
      $format.annotation:help($help_topics, "inventory")};
    const topic_document = $event:mk_info(player, $format.list:mk(topics)):resolve(player);
    for format in ({'text_html, 'text_djot})
      const rendered = $document:render(topic_document, format);
      $test_utils:assert_true(index(rendered['body], "reference-columns"),
        "The renderer gives topic references a compact layout");
      $test_utils:assert_eq(length(rendered['annotations]), 2, "Columns retain both topic actions");
      for ordinary in ({list, $format.list:mk(topics, true),
          $format.list:mk({$format.block:mk(topics[1], "Explanation."), topics[2]}),
          $format.list:mk({$format.paragraph:inline(topics[1], " with an explanation"), topics[2]}),
          $format.list:mk({" "})})
        const document = $event:mk_info(player, ordinary):resolve(player);
        $test_utils:assert_false(index($document:render(document, format)['body], "reference-columns"),
          "Prose, numbered lists, and multi-block items retain their normal layout");
      endfor
    endfor
    $test_utils:assert_eq($document:render(topic_document, 'text_plain)['body],
      "- look\n\n- inventory", "Plain references retain their order without layout markup");
    return true;
  endverb
  verb test_command_entries (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Command listings bind the receiver and reuse authored completion scope without executing anything.";
    const usage = this:_descriptor($help_utils:command_usage("get <thing>", player, {"this", "none", "none"}));
    $test_utils:assert_eq(usage["template"], "get {dobj}", "authored usage does not bind whichever object defines get nearby");
    $test_utils:assert_eq(usage["arguments"]["dobj"]["suggestions"]["source"], "nearby", "usage has normal completion");
    const target = create($container, player);
    target.open = true;
    const actions = target:inspection_commands(player);
    const cases = {
      {{"get take", $thing, "this", "none", "none"}, "get " + tostr(target), "", ""},
      {{"lock", $container, "this", "with", "any"}, "lock " + tostr(target) + " with {iobj}", "iobj", "nearby"},
      {{"get take steal grab", $container, "any", "from", "this"}, "get {dobj} from " + tostr(target), "dobj", "contents"},
      {{"put", $container, "any", "any", "this"}, "put {dobj} in " + tostr(target), "dobj", "inventory"},
      {{"give", $player, "any", "at", "any"}, "give {dobj} at {iobj}", "dobj", "nearby"}
    };
    try
      for example in (cases)
        const {spec, command, slot, scope} = example;
        const entry = $obj_utils:command_entry(target, spec, player, "", actions);
        const rendered = $document:render($event:mk_info(player, $format.list:mk({entry})):resolve(player), 'text_html);
        $test_utils:assert_eq(length(rendered['annotations]), 2, "invocation and source render together");
        const html = rendered['body];
        $test_utils:assert_true(index(html, "data-moor-annotation"), "command list serializes as HTML");
        const descriptor = this:_descriptor(flycontents(entry)[1]);
        $test_utils:assert_eq(descriptor["kind"], "command", "primary link invokes");
        $test_utils:assert_eq(descriptor[slot ? "template" | "command"], command, "command uses canonical spelling and exact target");
        if (slot)
          $test_utils:assert_eq(descriptor["arguments"][slot]["suggestions"]["source"], scope, "completion scope is retained");
        endif
        $test_utils:assert_eq(this:_descriptor(flycontents(entry)[3])["kind"], "verb", "programmer source is independent");
        $test_utils:assert_eq(this:_descriptor(flycontents(entry)[3])["receiver"], $url_utils:to_curie_str(target), "source keeps receiver");
        const ordinary = $obj_utils:command_entry(target, spec, #90102, "", actions);
        $test_utils:assert_eq(typeof(ordinary), TYPE_FLYWEIGHT, "nonprogrammers get only the invocation");
        for command_entry in ({entry, ordinary})
          const document = $event:mk_info(player, $format.list:mk({command_entry})):resolve(player);
          for format in ({'text_html, 'text_djot})
            $test_utils:assert_true(index($document:render(document, format)['body], "reference-columns"),
              "Command lists use columns with or without programmer source links");
          endfor
        endfor
      endfor
    finally
      recycle(target);
    endtry
    return true;
  endverb
  method test_argument_free_command_entries owner: ARCH_WIZARD
    "Actual command signatures must not invent argument fields for ignored input.";
    const cases = {
      {$sittable, "sit", "none", "this", "sit on top of " + tostr($sittable)},
      {$sittable, "stand", "none", "this", "stand out of " + tostr($sittable)},
      {$player, "inventory", "none", "none", "inventory"},
      {$player, "dms", "none", "none", "dms"},
      {$player, "listgag", "none", "none", "@listgag"},
      {$prog_features, "@ps", "none", "none", "@ps"},
      {$prog_features, "@schedules", "none", "none", "@schedules"},
      {$prog_features, "@codepaste", "none", "none", "@codepaste"},
      {$social_features, "frown", "none", "none", "frown"},
      {$social_features, "applaud", "none", "none", "applaud"}
    };
    for example in (cases)
      const {target, name, direct, indirect, command} = example;
      const {dobj_spec, prep, iobj_spec} = verb_args(target, name);
      $test_utils:assert_eq({dobj_spec, iobj_spec}, {direct, indirect}, "Command signature consumes only meaningful arguments");
      const spec = {verb_info(target, name)[3], target, dobj_spec, prep, iobj_spec};
      const entry = $obj_utils:command_entry(target, spec, #90102, "", {});
      const descriptor = this:_descriptor(entry);
      $test_utils:assert_eq(descriptor["command"], command, "Invoker receives a complete command");
      $test_utils:assert_false(maphaskey(descriptor, "arguments"), "Invoker does not request an unused argument");
    endfor
    return true;
  endmethod
  method test_action_semantics owner: ARCH_WIZARD
    "Aliases share authored meanings; unknown commands do not acquire an icon by guesswork.";
    const give = $actor:command_action("hand", "Mr. Welcome");
    $test_utils:assert_eq(give, ["label" -> "Give", "title" -> "Give to Mr. Welcome", "arguments" -> ["dobj" -> "Item"]], "alias resolves the defining verb's authored action");
    $test_utils:assert_false(maphaskey($root:command_action("open"), "icon"), "matching a familiar name does not invent semantics on another definer");
    $test_utils:assert_eq(this:_descriptor($format.annotation:object(player))["objectKind"], "person", "actor references retain person identity");
    $test_utils:assert_eq(this:_descriptor($format.annotation:object($container))["objectKind"], "container", "container references retain their identity");
    const entry = $obj_utils:command_entry($actor, {"give hand", $actor, "any", "at/to", "this"}, #90102);
    $test_utils:assert_eq(this:_descriptor(entry)["action"]["label"], "Give", "invocation carries the authored action");
    $test_utils:assert_eq(this:_descriptor(entry)["template"], "give {dobj} at " + tostr($actor), "action labels do not rewrite parser commands");
    $test_utils:assert_eq($document:render($event:mk_info(player, entry):resolve(player), 'text_plain)['body], "give <item> at " + $actor:name(), "caption names the argument using the exact parser preposition");
    const actor = $actor:create();
    add_verb(actor, {player, "rd", "offer hand"}, {"any", "at", "this"});
    set_verb_metadata(actor, "offer", "ui", ["label" -> "Offer", "icon" -> "give"]);
    $test_utils:assert_eq(actor:command_action("hand")["icon"], "give", "optional icon comes from verb metadata through aliases");
    $test_utils:assert_false(maphaskey($actor:command_action("give"), "icon"), "stock commands need not have icons");
    set_verb_metadata(actor, "offer", "ui", 42);
    $test_utils:assert_eq(actor:command_action("offer")["label"], "Offer", "malformed optional hints fall back to text");
    recycle(actor);
    return true;
  endmethod
endobject
