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
      input.ambient_passages = {{"an east door", 'fragment, "east"}, {"light shines", 'sentence}};
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
      $test_utils:assert_true(view['things][1]['integrated], "integrated object remains in panel state");
    finally
      recycle(item);
      recycle(room);
    endtry
    return true;
  endmethod
endobject
