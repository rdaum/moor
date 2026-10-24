object LOOK_SCENARIOS
  name: "Captured Look Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property calls (owner: ARCH_WIZARD, flags: "r") = 0;
  property secret (owner: ARCH_WIZARD, flags: "") = "private room prose";

  method resolve_blocks owner: ARCH_WIZARD
    "Count description resolution and exercise explicit viewer perspective and authority.";
    const {context} = args;
    if (this.mode == 'private)
      set_task_perms(caller_perms());
      return $format:literal_blocks(this.secret);
    endif
    #90124.calls = #90124.calls + 1;
    return {$format.paragraph:mk("Hello ", $sub:n(), "."),
      $format.paragraph:mk($document_node:mk('strong, $format:literal_inline("brass")), " bell.")};
  endmethod

  method _capture_as owner: ARCH_WIZARD
    "Run look collection and resolution as an explicit principal.";
    const {target, viewer} = args;
    set_task_perms(viewer);
    return target:look_self():capture(viewer);
  endmethod

  method test_capture_is_shared_and_inert owner: ARCH_WIZARD
    "A captured view retains content, labels, and identities across edits and object recycling.";
    const room = create($room);
    const item = create($thing);
    const person = create($actor);
    try
      room.name = "old room";
      item.name = "old key";
      person.name = "old actor";
      room.description = $format:producer_blocks(<#90124, .mode = 'count>);
      move(item, room);
      move(person, room);
      this.calls = 0;
      const view = room:look_self():capture(#90102);
      $test_utils:assert_eq(this.calls, 1, "description resolves once");
      $test_utils:assert_eq(view['title_text], "old room", "title is a captured field");
      const expected = $document:render(view['description], 'text_html)['body];
      $test_utils:assert_true(index($document:render(view['description], 'text_plain)['body], "Hello you.") > 0, "capture uses the supplied viewer");
      const before = $look:panel(view);
      const narrative = $look:narrative(view);
      const document = narrative:resolve(#90102);
      const original = $document:render(document, 'text_html);
      $test_utils:assert_true(index(before["content"], expected) > 0, "panel uses the captured document");
      const description_blocks = $document:encode(view['description])["blocks"];
      const narrative_blocks = $document:encode(document)["blocks"];
      $test_utils:assert_eq(narrative_blocks[2..length(description_blocks) + 1], description_blocks,
        "narrative uses the same captured blocks, independent of serialization anchors");
      $test_utils:assert_true(index(before["content"], "old key") > 0, "panel has captured item label");
      $test_utils:assert_true(index(original['body], "old key") > 0, "narrative has the same label");
      for descriptor in (mapvalues(before["annotations"]))
        $test_utils:assert_true(descriptor in mapvalues(original['annotations]) != 0,
          "layouts share reference descriptors");
      endfor
      room.name = "new room";
      item.name = "new key";
      person.name = "new actor";
      room.description = "new description";
      recycle(item);
      recycle(person);
      recycle(room);
      $test_utils:assert_eq($look:panel(view), before, "panel never rereads recycled objects");
      const replayed = $look:narrative(view):resolve(#90102);
      $test_utils:assert_eq($document:render(replayed, 'text_html), original,
        "narrative never rereads recycled objects");
      $test_utils:assert_eq(this.calls, 1, "layout and rendering do not rerun content producers");
    finally
      valid(item) && recycle(item);
      valid(person) && recycle(person);
      valid(room) && recycle(room);
    endtry
    return true;
  endmethod

  method test_narrative_keeps_authored_paragraphs_and_groups_room_details owner: ARCH_WIZARD
    "Room summaries use single line breaks without collapsing the author's paragraphs.";
    const room = create($room);
    const item = create($thing);
    const actor = create($actor);
    try
      room.name = "Atrium";
      room.description = {$format.paragraph:mk("First paragraph."),
        $format.paragraph:mk("Second paragraph.")};
      item.name = "brass key";
      actor.name = "Henri";
      move(item, room);
      move(actor, room);
      let input = room:look_self();
      input.exit_options = {['label -> "east", 'caption -> "east", 'descriptor -> [],
        'state -> 'open, 'destination -> #-1, 'ambient -> false]};
      const view = input:capture(#90102);
      const document = $look:narrative(view):resolve(#90102);
      const expected = "Atrium\n\nFirst paragraph.\n\nSecond paragraph.\n\n"
        + "You can go east.\nYou see a brass key here.\nHenri is here.";
      $test_utils:assert_eq($document:render(document, 'text_plain)['body], expected,
        "Description paragraphs stay separate; generated room details share a paragraph");
      const html = $document:render(document, 'text_html)['body];
      $test_utils:assert_true(index(html, "<p>First paragraph.</p>\n<p>Second paragraph.</p>") > 0,
        "The author's paragraph structure is preserved for web clients too");
    finally
      recycle(item);
      recycle(actor);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_long_actor_lists_are_summarized owner: ARCH_WIZARD
    "Past the listing limit a room names the first few actors and counts the rest.";
    const room = create($room);
    let actors = {};
    try
      room.name = "Crowded Hall";
      for i in [1..10]
        const actor = create($actor);
        actor.name = "Guest" + tostr(i);
        move(actor, room);
        actors = {@actors, actor};
      endfor
      const view = room:look_self():capture(#90102);
      const body = $document:render($look:narrative(view):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(body, " and 5 others are here."), "remaining actors are counted");
      $test_utils:assert_eq(length({actor for actor in (actors) if (index(body, actor.name + " ") || index(body, actor.name + ","))}), 5,
        "only the first five names are shown");
      for actor in (actors[1..2])
        recycle(actor);
      endfor
      actors = actors[3..$];
      const short = $document:render($look:narrative(room:look_self():capture(#90102)):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(short, " are here.") && !index(short, "others"), "a list within the limit names everyone");
    finally
      for actor in (actors)
        valid(actor) && recycle(actor);
      endfor
      recycle(room);
    endtry
    return true;
  endmethod

  method test_capture_preserves_authority owner: ARCH_WIZARD
    "Public capture cannot elevate the caller through private description producers.";
    const room = create($room);
    try
      room.description = "public prose";
      const public_view = this:_capture_as(room, #90102);
      $test_utils:assert_eq($document:render(public_view['description], 'text_plain)['body],
        "public prose", "public capture succeeds with reduced authority");
      room.description = $format:producer_blocks(<#90124, .mode = 'private>);
      $test_utils:assert_raises(E_PERM, this, "_capture_as", {room, #90102},
        "private producer reads use the capturing principal");
    finally
      recycle(room);
    endtry
    return true;
  endmethod

  method test_ambient_and_integrated_content owner: ARCH_WIZARD
    "Ambient passage links, integrated prose, and action labels are shared by both layouts.";
    const room = create($room);
    const item = create($thing);
    try
      room.name = "hall";
      room.description = "A hall.";
      item.name = "bell";
      item.integrated_description = "A bell hangs here";
      move(item, room);
      let input = room:look_self();
      input.exit_options = {['label -> "east", 'caption -> "east", 'descriptor -> [],
        'state -> 'open, 'destination -> #-1, 'ambient -> true]};
      input.ambient_passages = {{"an east door", 'fragment, "east"}, {"Light shines.", 'sentence}};
      input.actions = {{"ring bell", "Ring *bell*"}};
      const view = input:capture(#90102);
      const narrative = $document:render($look:narrative(view):resolve(#90102), 'text_plain)['body];
      const panel = $look:panel(view, 'text_plain)["content"];
      for output in ({narrative, panel})
        $test_utils:assert_true(index(output, "A bell hangs here.") > 0, "integrated prose is retained");
        $test_utils:assert_true(index(output, "You see an east door.") > 0, "ambient fragment is retained");
        $test_utils:assert_true(index(output, "Light shines.") > 0, "ambient sentence is retained");
        $test_utils:assert_true(index(output, "Ring *bell*") > 0, "action labels remain literal");
      endfor
      $test_utils:assert_eq(length(view['exits]), 1, "ambient exit appears in captured exits");
      $test_utils:assert_false(index(narrative, "You can go "), "Described exits are not listed twice");
      input.exit_options = {@input.exit_options,
        ['label -> "out", 'caption -> "out", 'descriptor -> [],
          'state -> 'open, 'destination -> #-1, 'ambient -> false],
        ['label -> "examples", 'caption -> "examples", 'descriptor -> [],
          'state -> 'open, 'destination -> room, 'ambient -> false]};
      const mixed = $document:render($look:narrative(input:capture(#90102)):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(mixed, "You can go out or to hall.") > 0,
        "Only undescribed exits need a list, with labels rather than direction grammar");
      $test_utils:assert_true(view['things][1]['integrated], "integrated object remains in panel state");
    finally
      recycle(item);
      recycle(room);
    endtry
    return true;
  endmethod
  method test_fixed_objects_do_not_offer_get_and_drop owner: ARCH_WIZARD
    "Command help omits $thing's get and drop for objects that cannot be carried.";
    const fixture = create($thing);
    try
      fixture.name = "marble fountain";
      const names = fn(target)
        let listed = {};
        for entry in ($obj_utils:collect_targetable_verbs({target}))
          for line in (entry["verbs"])
            listed = {@listed, toliteral(line)};
          endfor
        endfor
        return listed:join(" ");
      endfn;
      $test_utils:assert_true(index(names(fixture), "get"), "a portable thing offers get");
      fixture.portable = false;
      const fixed = names(fixture);
      $test_utils:assert_false(index(fixed, "\"get ") || index(fixed, "drop"), "a fixed thing offers neither get nor drop");
    finally
      recycle(fixture);
    endtry
    return true;
  endmethod

  method test_locked_way_is_open_to_a_key_holder owner: ARCH_WIZARD
    "A locked passage reads as locked to most viewers and as open to one who carries its key.";
    const area = create($area);
    const room = create($room);
    const destination = create($room);
    const key = create($thing);
    try
      move(room, area);
      move(destination, area);
      const condition = ['kind -> 'predicate, 'name -> 'is, 'bind -> [],
        'arguments -> ['left -> ['kind -> 'input, 'name -> 'Key], 'right -> ['kind -> 'literal, 'value -> key]]];
      const rule = $rule:mk('unlock_rule, condition, $rule_engine:passage_scope());
      area:create_passage(room, destination,
        $passage:mk(room, "east", {}, "", false, destination, "west", {}, "", false)
        :with_unlock_rule(rule):with_locked(true));
      $test_utils:assert_eq($room:exit_options(room, #90102)[1]['state], 'locked, "no key: locked");
      move(key, #90102);
      $test_utils:assert_eq($room:exit_options(room, #90102)[1]['state], 'open, "carried key: open");
    finally
      recycle(key);
      recycle(room);
      recycle(destination);
      recycle(area);
    endtry
    return true;
  endmethod

  method test_travel_sentence_uses_captured_names_and_commands owner: ARCH_WIZARD
    "Describe named destinations while preserving the exit command and captured wording.";
    const area = create($area);
    const room = create($room);
    const destination = create($room);
    try
      move(room, area);
      move(destination, area);
      destination.name = "Passage Examples";
      let passage = area:create_passage(room, destination,
        $passage:mk(room, "examples", {}, "", false, destination, "out", {}, "", false));
      const view = room:look_self():capture(#90102);
      destination.name = "Renamed Gallery";
      const body = $document:render($look:narrative(view):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(body, "You can go to Passage Examples.") > 0,
        "A named exit uses its destination's captured noun phrase");
      $test_utils:assert_eq(view['exits][1]['travel]['descriptor]["command"], "go examples",
        "A descriptive link still runs the original exit command");
      passage = area:update_passage(room, destination,
        passage:with_departure_phrase_from(room, "through the blue arch"));
      let current = room:look_self():capture(#90102);
      $test_utils:assert_eq(current['exits][1]['travel]['label], "through the blue arch",
        "Authored travel wording takes precedence over the destination name");
      area:update_passage(room, destination, passage:with_open(false));
      current = room:look_self():capture(#90102);
      const closed = $document:render($look:narrative(current):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(closed, "The way through the blue arch is closed.") > 0,
        "A closed way is described without claiming that the player can travel");
      $test_utils:assert_false(index(closed, "You can go "), "Closed exits are not offered as open ways");
      let input = room:look_self();
      input.exit_options = {['label -> "misty arch", 'caption -> "misty arch", 'descriptor -> [],
        'commands -> {"enter misty arch"}, 'state -> 'open, 'destination -> #-1, 'ambient -> false]};
      const hidden = $document:render($look:narrative(input:capture(#90102)):resolve(#90102), 'text_plain)['body];
      $test_utils:assert_true(index(hidden, "You can go by typing \"enter misty arch\".") > 0,
        "An unknown destination shows its actual command without inventing a destination");
    finally
      recycle(room);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod

  method test_passage_description_formats_and_references owner: ARCH_WIZARD
    "All supported source formats render for one viewer without inferring links from direction words.";
    const area = create($area);
    const room = create($room);
    const destination = create($room);
    try
      move(room, area);
      move(destination, area);
      room.description = "A quiet hall.";
      let passage = area:create_passage(room, destination,
        $passage:mk(room, "in", {}, "", true, destination, "out", {}, "", true));
      const descriptions = {
        {"inside the hall, a brass door waits.", 'literal, "inside the hall, a brass door waits."},
        {"A {brass} door waits.", 'literal, "A {brass} door waits."},
        {"A *brass* door waits.", 'djot_inline, "A brass door waits."},
        {"An {direction} door waits.", 'template, "An in door waits."},
        {$sub_utils:compile("An {direction} arch waits."), 'literal, "An in arch waits."}
      };
      for example in (descriptions)
        const {source, kind, expected} = example;
        passage = area:update_passage(room, destination, passage:with_description_from(room, source, kind));
        const view = room:look_self():capture(#90102);
        const text = $document:render(view['description], 'text_plain)['body];
        $test_utils:assert_true(index(text, expected) > 0, "Capture preserves the authored sentence and resolves its bindings");
        const html = $document:render(view['description], 'text_html);
        const links = mapvalues(html['annotations]);
        $test_utils:assert_eq(length(links), 1, "The complete description has one exit reference");
        $test_utils:assert_eq(links[1]["exit"]["passage"], passage.connection_id, "The reference identifies the installed connection");
        const narrative = $document:render($look:narrative(view):resolve(#90102), 'text_plain)['body];
        $test_utils:assert_false(index(narrative, "You can go "),
          "A linked description replaces the separate narrative exit listing");
        if (kind == 'djot_inline)
          $test_utils:assert_true(index(html['body], "<strong>brass</strong>") > 0, "Djot emphasis survives HTML rendering");
          $test_utils:assert_true(index($document:render(view['description], 'text_djot)['body], "*brass*") > 0,
            "The same content retains emphasis in Djot");
        endif
      endfor
      const phrase = $format.annotation:exit(room, "in", "brass door");
      passage = area:update_passage(room, destination,
        passage:with_description_from(room, {"A ", phrase, " stands beside the fireplace."}, 'literal));
      const explicit = room:look_self():capture(#90102);
      const references = mapvalues($document:render(explicit['description], 'text_html)['annotations]);
      $test_utils:assert_eq(length(references), 1, "An authored reference is not wrapped in a second interactive reference");
      const departure = $passage:movement_from(passage, #90102, room, destination):resolve(#90102);
      $test_utils:assert_eq($document:render(departure, 'text_plain)['body], "You head in.",
        "Movement wording does not reuse an ambient description or its formatting");
      const phrased = passage:with_departure_phrase_from(room, "through the brass door");
      const movement = $passage:movement_from(phrased, #90102, room, destination):resolve(#90102);
      $test_utils:assert_eq($document:render(movement, 'text_plain)['body], "You head through the brass door.",
        "The dedicated departure phrase controls movement and its preview");
      const before = passage;
      $test_utils:assert_raises(E_INVARG, passage, "with_description_from", {room, "# heading", 'djot_inline},
        "Block syntax is rejected before saving inline exit prose");
      $test_utils:assert_eq(area:passage_for(room, destination), before, "A rejected description preserves installed state");
    finally
      recycle(room);
      recycle(destination);
      area:destroy();
    endtry
    return true;
  endmethod
endobject
