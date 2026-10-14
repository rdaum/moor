object HEADLESS_SITTABLE_SCENARIOS
  name: "Owned Furniture Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true
  override import_export_id = "headless_sittable_scenarios";
  override import_export_hierarchy = {"tests", "headless"};

  method _as owner: ARCH_WIZARD
    "Call a furniture operation as the specified fixture principal.";
    caller == this || raise(E_PERM);
    const {principal, target, operation, @values} = args;
    set_task_perms(principal);
    return target:(operation)(@values);
  endmethod

  method test_owned_seats_and_actor_authority owner: ARCH_WIZARD
    "Ordinary owners can seat NPCs and trigger reactions without controlling other actors.";
    const room = create($room, #90100, 2);
    const seat = create($sittable, #90101, 2);
    const other = create($sittable, #90102, 2);
    const actor = create($actor, #90101, 2);
    const stranger = create($actor, #90102, 2);
    try
      for object in ({seat, other, actor, stranger})
        move(object, room);
      endfor
      $reaction:declare_state(seat, 'visits, 'integer, 0);
      $reaction:install(seat, "visit_reaction", $reaction:mk('on_sit, 0,
        {{'increment, 'visits}}, $reaction:state_types(seat)));
      $test_utils:assert_true(this:_as(#90101, seat, "add_sitter", actor), "An owned NPC can sit");
      $test_utils:assert_eq({seat.sitting, seat.visits, room:get_engagement(actor)},
        {{actor}, 1, seat}, "The ordinary installation responds and the room records the seat");
      for operation in ({"remove_sitter", "remove_occupant", "dump_sitter", "on_location_exit"})
        const result = `this:_as(#90102, seat, operation, actor) ! E_PERM => E_PERM';
        $test_utils:assert_eq(result, E_PERM, "A foreign caller cannot unseat this actor");
      endfor
      $test_utils:assert_eq(`this:_as(#90101, seat, "add_sitter", stranger) ! E_PERM => E_PERM',
        E_PERM, "Owning the furniture does not authorize acting as another person's NPC");
      $test_utils:assert_eq(seat.sitting, {actor}, "Denied operations preserve the seat");
      $test_utils:assert_eq(`this:_as(#90102, room, "engage_actor", actor, other)
        ! E_PERM => E_PERM', E_PERM, "Room engagement cannot bypass actor authority");
      $test_utils:assert_true(this:_as(#90101, other, "add_sitter", actor), "An NPC can switch seats");
      $test_utils:assert_eq({seat.sitting, other.sitting, room:get_engagement(actor)},
        {{}, {actor}, other}, "Switching clears the old seat and records the new one");
      $test_utils:assert_true(this:_as(#90101, other, "remove_sitter", actor), "The owner can stand its NPC");
      $test_utils:assert_eq({other.sitting, room:get_engagement(actor)}, {{}, #-1},
        "Standing clears the room's engagement too");
    finally
      for object in ({actor, stranger, seat, other, room})
        recycle(object);
      endfor
    endtry
    return true;
  endmethod

  method test_seats_survive_rejected_furniture_moves owner: ARCH_WIZARD
    "Rejected moves preserve seats; owner and delegated moves clear them after moving.";
    const room = create($room, #90100, 2);
    const destination = create($room, #90100, 2);
    const closed = create($root, #90100, 2);
    const seat = create($sittable, #90101, 2);
    const actor = create($actor, #90101, 2);
    try
      move(seat, room);
      move(actor, room);
      this:_as(#90101, seat, "add_sitter", actor);
      $test_utils:assert_eq(`this:_as(#90102, seat, "moveto", destination) ! E_PERM => E_PERM',
        E_PERM, "A stranger cannot move the furniture");
      $test_utils:assert_eq(this:_as(#90101, seat, "moveto", closed), E_NACC,
        "Destination rejection is returned to the owner");
      $test_utils:assert_eq({seat.location, seat.sitting, room:get_engagement(actor)},
        {room, {actor}, seat}, "Neither rejected move unseats the actor");
      this:_as(#90101, seat, "moveto", destination);
      $test_utils:assert_eq({seat.location, seat.sitting, room:get_engagement(actor)},
        {destination, {}, #-1}, "The successful move empties the furniture");
      this:_as(#90101, seat, "moveto", room);
      this:_as(#90101, seat, "add_sitter", actor);
      const key = this:_as(#90101, $root, "issue_capability", seat, {'move});
      this:_as(#90102, key, "moveto", destination);
      $test_utils:assert_eq({seat.location, seat.sitting, room:get_engagement(actor)},
        {destination, {}, #-1}, "A delegated move clears the real furniture and its room record");
    finally
      for object in ({actor, seat, closed, destination, room})
        recycle(object);
      endfor
    endtry
    return true;
  endmethod
endobject
