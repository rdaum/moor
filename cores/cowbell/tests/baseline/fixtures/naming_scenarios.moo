object NAMING_SCENARIOS
  name: "Naming Scenarios"
  parent: THING
  owner: ARCH_WIZARD
  readable: true

  property require_secret (owner: ARCH_WIZARD, flags: "rc") = false;
  property secret_label (owner: ARCH_WIZARD, flags: "") = "private";
  property keeper (owner: ARCH_WIZARD, flags: "rc") = #-1;
  property keeper_phrase (owner: ARCH_WIZARD, flags: "rc") = "the box you left here";
  property collective (owner: ARCH_WIZARD, flags: "rc") = false;
  property extra_match_name (owner: ARCH_WIZARD, flags: "rc") = "";
  property count_phrases (owner: ARCH_WIZARD, flags: "rc") = false;
  property phrase_calls (owner: ARCH_WIZARD, flags: "rc") = [];

  method noun_phrase owner: ARCH_WIZARD
    "Customize one phrase while retaining the inherited forms and listing dispatch.";
    set_task_perms(caller_perms());
    if (this.require_secret)
      const secret = this.secret_label;
    endif
    const {form, viewer} = args;
    if (this.count_phrases)
      "Test instrumentation measures repeated queries; ordinary naming verbs do not mutate.";
      let calls = this.phrase_calls;
      calls[form] = (maphaskey(calls, form) ? calls[form] | 0) + 1;
      this.phrase_calls = calls;
    endif
    if (form == 'definite && viewer == this.keeper)
      return this.keeper_phrase;
    endif
    return pass(@args);
  endmethod

  method noun_grammar owner: ARCH_WIZARD
    "A prototype can compute number without changing its stored settings.";
    set_task_perms(caller_perms());
    if (this.require_secret)
      const secret = this.secret_label;
    endif
    let grammar = pass(@args);
    this.collective && (grammar['number] = 'plural);
    return grammar;
  endmethod

  method match_names owner: ARCH_WIZARD
    "Contribute a computed name through the matching interface.";
    set_task_perms(caller_perms());
    if (this.require_secret)
      const secret = this.secret_label;
    endif
    const names = pass(@args);
    return this.extra_match_name ? {@names, this.extra_match_name} | names;
  endmethod

  method _set_as owner: ARCH_WIZARD
    "Exercise the public setter with the specified caller authority.";
    const {target, principal, changes} = args;
    set_task_perms(principal);
    return target:set_grammar(changes);
  endmethod

  method test_noun_forms owner: ARCH_WIZARD
    "Count, mass, complete names, and pronunciation overrides produce the intended phrases.";
    const item = create($thing);
    try
      for example in ({
        {"prototype box", 'count, 'singular, "", "a prototype box", "the prototype box"},
        {"apple", 'count, 'singular, "", "an apple", "the apple"},
        {"boots", 'count, 'plural, "", "some boots", "the boots"},
        {"water", 'mass, 'singular, "", "some water", "the water"},
        {"Henri", 'name, 'singular, "", "Henri", "Henri"},
        {"Ada's mailbox", 'name, 'singular, "", "Ada's mailbox", "Ada's mailbox"},
        {"The Who", 'name, 'plural, "", "The Who", "The Who"},
        {"MRI scanner", 'count, 'singular, "an", "an MRI scanner", "the MRI scanner"}})
        const {label, kind, number, article, indefinite, definite} = example;
        item.name = label;
        item:set_grammar(['kind -> kind, 'number -> number, 'article -> article]);
        $test_utils:assert_eq(item:noun_phrase('label, #90102), label, "Literal label");
        $test_utils:assert_eq(item:noun_phrase('bare, #90102), label, "Bare prose");
        $test_utils:assert_eq(item:noun_phrase('indefinite, #90102), indefinite, "Introduction");
        $test_utils:assert_eq(item:noun_phrase('definite, #90102), definite, "Identified object");
        $test_utils:assert_eq(item:noun_phrase('listing, #90102), indefinite, "Default listing");
        $test_utils:assert_eq(item:noun_phrase('definite, item), "you", "Viewer perspective");
        $test_utils:assert_eq(item:noun_phrase('label, item), label, "Viewer's literal label");
      endfor
      for example in ({{"hour", "an"}, {"honest answer", "an"}, {"user", "a"},
        {"unusual object", "an"}, {"unicycle", "a"}})
        $test_utils:assert_eq(example[1]:indefinite_article(), example[2], "Shared pronunciation");
      endfor
    finally
      recycle(item);
    endtry
    return true;
  endmethod

  method test_prototype_naming_overrides owner: ARCH_WIZARD
    "Listing dispatch honors phrase overrides and grammar can be computed on a prototype.";
    const item = create(this);
    const utility = create($root);
    try
      item.name = "box";
      item.keeper = #90102;
      item:set_grammar(['listing -> 'definite]);
      $test_utils:assert_eq(item:noun_phrase('listing, #90102), "the box you left here",
        "Listing calls the descendant's definite form");
      $test_utils:assert_eq(item:noun_phrase('definite, #90101), "the box", "Other viewer");
      $test_utils:assert_eq(item:noun_phrase('label, #90102), "box", "Literal name is separate");
      item.collective = true;
      $test_utils:assert_eq(item:noun_grammar(#90102)['number], 'plural, "Computed number");
      $test_utils:assert_eq(item.noun_number, 'singular, "Query does not change stored number");
      $test_utils:assert_eq(item:noun_phrase('indefinite, #90102), "some box", "Uses grammar verb");
      utility.name = "Object Matching Utilities";
      $test_utils:assert_eq(utility:noun_phrase('listing, #90102), utility.name,
        "Utility needs no listing property");
      $test_utils:assert_eq(`utility.listing_form ! E_PROPNF => E_PROPNF', E_PROPNF,
        "Listing does not create state");
    finally
      recycle(item);
      recycle(utility);
    endtry
    return true;
  endmethod

  method test_grammar_edits_and_authority owner: ARCH_WIZARD
    "Invalid edits are atomic; owner and explicit bearer grants can edit and restore inheritance.";
    const prototype = create($thing, #90101);
    const item = create(prototype, #90101);
    try
      this:_set_as(prototype, #90101, ['kind -> 'mass]);
      $test_utils:assert_eq(item:noun_grammar(#90102)['kind], 'mass, "Inherited setting");
      this:_set_as(item, #90101, ['kind -> 'count, 'number -> 'plural]);
      $test_utils:assert_eq(prototype.noun_kind, 'mass, "Instance edit preserves prototype");
      $test_utils:assert_raises(E_INVARG, this, "_set_as",
        {item, #90101, ['kind -> 'name, 'number -> 'several]},
        "Validate all fields before writing");
      $test_utils:assert_eq(item.noun_kind, 'count, "Invalid edit has no partial effects");
      $test_utils:assert_raises(E_PERM, this, "_set_as",
        {item, #90102, ['kind -> 'name]}, "Unrelated player cannot edit");
      const capability = item:issue_capability(item, {'set_grammar});
      this:_set_as(capability, #90102, ['kind -> 'name]);
      $test_utils:assert_eq(item.noun_kind, 'name, "Bearer grant permits a grammar edit");
      this:_set_as(capability, #90102, ['kind -> 'inherit, 'number -> 'inherit]);
      $test_utils:assert_true(is_clear_property(item, "noun_kind"), "Reset clears local value");
      $test_utils:assert_eq(item.noun_kind, 'mass, "Reset follows prototype");
    finally
      recycle(item);
      recycle(prototype);
    endtry
    return true;
  endmethod

  method test_message_capture_and_real_previews owner: ARCH_WIZARD
    "Templates call overrides once per form and previews retain the real host's computed grammar.";
    const item = create(this);
    try
      item.name = "guards";
      item.keeper = #90102;
      item.keeper_phrase = "the guards you hired";
      item.collective = true;
      item.count_phrases = true;
      const template = "{The d} {agree d name:are|is} here; {the same}.";
      const event = $event:mk_test(#90101, @$sub_utils:compile(template))
        :with_dobj(item):with_object_binding('same, item);
      const document = event:resolve(#90102);
      $test_utils:assert_eq($document:render(document, 'text_plain)['body],
        "The guards you hired are here; the guards you hired.", "Object phrase and agreement");
      $test_utils:assert_eq(item.phrase_calls['definite], 1, "Bindings share a captured answer");
      item.name = "changed label";
      item.keeper_phrase = "changed phrase";
      const html = $document:render(document, 'text_html);
      $test_utils:assert_true(index(html['body], "guards you hired") > 0,
        "A second format consumes captured text");
      $test_utils:assert_true(length(html['annotations]) > 0, "Object interaction survives");
      $test_utils:assert_eq(item.phrase_calls['definite], 1, "Formatting does not call the object");
      event:resolve(#90102);
      $test_utils:assert_eq(item.phrase_calls['definite], 2, "A new resolution captures new answers");
      item.name = "guards";
      const bindings = ['actor -> ['kind -> 'object, 'example -> 'actor],
        'dobj -> ['kind -> 'object, 'example -> 'host]];
      const preview = $sub_utils:preview("{The d} {agree d name:are|is} here.", bindings, item);
      $test_utils:assert_true(index(preview:join("\n"), "The guards are here.") > 0,
        "Preview calls the host's computed grammar, not a copy of its stored number");
    finally
      recycle(item);
    endtry
    return true;
  endmethod
  method test_article_matching_and_command_structure owner: ARCH_WIZARD
    "Shared matching preserves exact names, ordinals, explicit scopes, and raw command spans.";
    const box = create(this);
    const second = create($thing);
    const named = create($actor);
    try
      box.name = "prototype box";
      box.aliases = {"box"};
      box.extra_match_name = "enchanted coffer";
      second.name = "box";
      named.name = "The Who";
      const scope = {box, second, named};
      const options = ['fuzzy_threshold -> 0.0, 'context -> #90102];
      for reference in ({"the prototype box", "a prototype box", "an prototype box", "some prototype box", "the enchanted coffer"})
        $test_utils:assert_eq($match:candidates_in_scope(reference, scope, options), {box}, reference);
      endfor
      $test_utils:assert_eq($match:candidates_in_scope("a box", scope, options), {box, second}, "Indefinite preserves ambiguity");
      $test_utils:assert_eq($match:candidates_in_scope("the second box", scope, options), {second}, "Article before ordinal");
      $test_utils:assert_eq($match:candidates_in_scope("the wh", scope, options), {named}, "Partial complete name");
      named.name = "Henri";
      $test_utils:assert_eq($match:candidates_in_scope("the Henri", scope, options), {}, "No invented article on a complete name");
      named.name = "the second box";
      $test_utils:assert_eq($match:candidates_in_scope(named.name, scope, options), {named}, "Exact authored name precedes ordinal");
      const restricted = {{box, "public handle"}, box};
      $test_utils:assert_eq($match:candidates_in_scope("the public handle", restricted, options), {box}, "Explicit key accepts article");
      $test_utils:assert_eq($match:candidates_in_scope("enchanted coffer", restricted, options), {}, "Explicit keys replace hooks");
      $test_utils:assert_eq($match:candidates_in_scope("the " + tostr(named), {box}, options), {}, "Article cannot escape scope");
      box.name = "box in red";
      const command = "get \"the box in red\" from me";
      const parsed = $match:parse_command(command, scope, #90102, 0.0);
      const native = parse_command(command, {}, true, 0.0);
      $test_utils:assert_eq(parsed['dobj], box, "Quoted preposition remains inside reference");
      $test_utils:assert_eq(parsed['iobj], #90102, "Explicit context for me");
      for field in (mapkeys(native))
        if (!(field in {'dobj, 'iobj, 'ambiguous_dobj, 'ambiguous_iobj}))
          $test_utils:assert_eq(parsed[field], native[field], "Native command field " + tostr(field));
        endif
      endfor
      const ambiguous = $match:parse_command("get a box", {box, second}, #90102, 0.0);
      $test_utils:assert_eq(ambiguous['ambiguous_dobj], {box, second}, "Command ambiguity retained");
      const ordinary = $match:object_suggestions({box});
      $test_utils:assert_eq(length($match:rank_suggestions(ordinary, "the enchanted", 10)["items"]), 1,
        "Completion uses the prototype's matching names");
      const suggestions = $match:object_suggestions(restricted);
      $test_utils:assert_eq(length($match:rank_suggestions(suggestions, "the public h", 10)["items"]), 1,
        "Completion accepts articles around explicit keys");
      $test_utils:assert_eq(length($match:rank_suggestions(suggestions, "enchanted", 10)["items"]), 0,
        "Completion does not expose omitted names");
    finally
      recycle(box);
      recycle(second);
      recycle(named);
    endtry
    return true;
  endmethod
  method test_world_lists_and_stock_messages owner: ARCH_WIZARD
    "Room, clothing, and action prose use phrases, plural agreement, and complete reference nodes.";
    const room = create($room);
    const item = create(this);
    const actor = create($actor);
    const boots = create($wearable);
    try
      room.name = "Naming Test Room";
      item.name = "box";
      item.keeper = #90102;
      item.keeper_phrase = "the box beside you";
      item.listing_form = 'definite;
      actor.name = "guards";
      actor:set_grammar(['kind -> 'count, 'number -> 'plural]);
      boots.name = "boots";
      boots.noun_number = 'plural;
      actor.wearing = {boots};
      const emote = actor:mk_emote_event("wave.");
      $test_utils:assert_eq($document:render(emote:resolve(actor), 'text_plain)['body],
        "The guards wave.", "An emote retains third-person grammar for its author");
      const mixed = $event:mk_info(actor, $sub:attribution(actor), " / ", $sub:the('actor));
      $test_utils:assert_eq($document:render(mixed:resolve(actor), 'text_plain)['body],
        "The guards / you", "Neutral attribution and viewer references have separate captures");
      move(actor, room);
      move(item, room);
      const view = room:look_self():capture(#90102);
      const document = $look:narrative(view):resolve(#90102);
      const plain = $document:render(document, 'text_plain)['body];
      $test_utils:assert_true(index(plain, "You see the box beside you here.") > 0, "Room respects prototype phrase");
      $test_utils:assert_true(index(plain, "Some guards are here.") > 0, "Single collective actor takes plural agreement");
      const html = $document:render(document, 'text_html);
      $test_utils:assert_true(length(html['annotations]) >= 2, "Room preserves separate references");
      const clothing = actor:look_self():capture(#90102);
      const described = $document:render(clothing['description], 'text_html);
      $test_utils:assert_true(index(described['body], "some boots") > 0, "Clothing uses plural listing form");
      $test_utils:assert_true(length(described['annotations]) > 0, "Clothing keeps object links");
      for example in ({{$container.open_msg, "open"}, {$wearable.wear_msg, "put on"}, {$note.read_msg, "read"}})
        const event = $event:mk_info(actor, @example[1]):with_dobj(boots);
        $test_utils:assert_eq($document:render(event:resolve(#90102), 'text_plain)['body],
          "The guards " + example[2] + " the boots.", "Observer sees collective subject");
        $test_utils:assert_eq($document:render(event:resolve(actor), 'text_plain)['body],
          "You " + example[2] + " the boots.", "Actor perspective remains second person");
      endfor
      const denied = $event:mk_error(actor, @$container.already_closed_msg):with_dobj(boots);
      $test_utils:assert_eq($document:render(denied:resolve(actor), 'text_plain)['body],
        "The boots are already closed.", "Denials agree with the item, not the actor");
      room.name = "courtyard";
      room.noun_kind = 'count;
      const message = $dm:mk(actor, #90102, "Reporting for duty.");
      const received = message:display_event(#90102);
      $test_utils:assert_true(index(received.tts_text, "Direct message from the guards in the courtyard") == 1,
        "Spoken attribution uses noun phrases");
      $test_utils:assert_true(index($document:render(received:resolve(#90102), 'text_plain)['body],
        "guards (in the courtyard)") == 1, "Sender heading stays literal; location is a phrase");
      const reply = $dm:mk(#90102, actor, "Thank you."):sender_echo_event();
      $test_utils:assert_true(index(reply.tts_text, "Message sent to the guards") == 1,
        "Spoken receipt names its recipient grammatically");
    finally
      recycle(boots);
      recycle(item);
      recycle(actor);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_legacy_naming_conversion owner: ARCH_WIZARD
    "Conversion retains inheritance and letter subjects, reports custom code, and rejects stale records.";
    const prototype = create($thing);
    const child = create(prototype);
    const letter = create($letter);
    try
      prototype.name = "an old flask";
      add_property(prototype, "is_proper_noun_name", false, {this.owner, "rc"});
      add_property(prototype, "is_countable_noun", false, {this.owner, "rc"});
      add_property(prototype, "is_plural_noun", false, {this.owner, "rc"});
      prototype.indefinite_article = "an";
      child.name = "water";
      letter.name = "Greetings from home";
      const snapshot = $obj_utils:capture_legacy_naming({prototype, child, letter});
      $test_utils:assert_true(length(snapshot['review]) > 0, "Article-bearing name is reported");
      $test_utils:assert_eq(letter.subject, "", "Capture does not mutate a letter");
      delete_property(prototype, "is_proper_noun_name");
      delete_property(prototype, "is_countable_noun");
      delete_property(prototype, "is_plural_noun");
      $obj_utils:apply_legacy_naming(snapshot['records]);
      $test_utils:assert_eq(prototype.noun_kind, 'mass, "Countability converts to mass kind");
      $test_utils:assert_true(is_clear_property(child, "noun_kind"), "An inherited choice stays inherited");
      $test_utils:assert_eq(child:noun_phrase('indefinite, #90102), "some water", "Child uses converted prototype");
      $test_utils:assert_eq({letter.name, letter.subject}, {"letter", "Greetings from home"}, "Subject preserved before rename");
      $obj_utils:apply_legacy_naming(snapshot['records]);
      $test_utils:assert_eq(prototype.name, "an old flask", "No guessing about articles in names");
      prototype.name = "changed flask";
      const denied = `$obj_utils:apply_legacy_naming(snapshot['records]) ! E_INVARG => E_INVARG';
      $test_utils:assert_eq(denied, E_INVARG, "Renamed targets require review");
      add_verb(prototype, {this.owner, "rxd", "is_countable"}, {"this", "none", "this"});
      set_verb_code(prototype, "is_countable", {"return false;"});
      const custom = $obj_utils:capture_legacy_naming({prototype});
      $test_utils:assert_true(length(custom['review][1]['verbs]) > 0, "Custom old verb is reported");
    finally
      recycle(child);
      recycle(prototype);
      recycle(letter);
    endtry
    return true;
  endmethod
  method _query_as owner: ARCH_WIZARD
    "Exercise naming queries with a principal different from the viewer.";
    const {principal, target, query} = args;
    set_task_perms(principal);
    if (query == 'parse)
      return $match:parse_command("get the box", {target}, #90100, 0.0);
    endif
    return $event:mk_info(principal, $sub:reference(target)):resolve(#90100);
  endmethod

  method test_naming_queries_preserve_authority owner: ARCH_WIZARD
    "Rendering and parsing cannot borrow the viewer's or utility owner's access to naming data.";
    const item = create(this, #90101);
    try
      item.name = "box";
      item.require_secret = true;
      for query in ({'parse, 'render})
        $test_utils:assert_raises(E_PERM, this, "_query_as", {#90102, item, query},
          "Naming query retains its caller's permissions");
      endfor
      item.require_secret = false;
      const result = this:_query_as(#90102, item, 'parse);
      $test_utils:assert_eq(result['dobj], item, "Public naming succeeds with the same principal");
    finally
      recycle(item);
    endtry
    return true;
  endmethod
endobject
