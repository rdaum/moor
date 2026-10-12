object HEADLESS_OBJECT_SCENARIOS
  name: "Headless Object Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for object creation, mutation, movement, and object identity.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_object_scenarios";

  verb test_headless_object_lifecycle (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: persistent object creation, mutation, movement, and destruction work without player I/O.";
    room = #-1;
    item = #-1;
    try
      room = $room:create();
      item = $thing:create();
      room:set_name_aliases("headless test room", {"headless-room"});
      item:set_name_aliases("headless test token", {"headless-token"});
      item:set_description($sub_utils:compile("A {n} marker for headless runtime tests."));
      $test_utils:assert_true(valid(room), "created room should be valid");
      $test_utils:assert_true(valid(item), "created item should be valid");
      $test_utils:assert_eq(parent(room), $room, "created room should inherit from $room");
      $test_utils:assert_eq(parent(item), $thing, "created item should inherit from $thing");
      $test_utils:assert_eq(item:name(), "headless test token", "set_name_aliases should update name");
      $test_utils:assert_eq(item:aliases(), {"headless-token"}, "set_name_aliases should update aliases");
      $test_utils:assert_type(item.description, TYPE_LIST, "explicit substitutions remain structured content");
      item:moveto(room);
      $test_utils:assert_eq(item.location, room, "moveto should update item location");
      $test_utils:assert_true(item in room:contents(), "moveto should update destination contents");
      item:destroy();
      $test_utils:assert_false(valid(item), "destroy should recycle item");
      item = #-1;
      room:destroy();
      $test_utils:assert_false(valid(room), "destroy should recycle room");
      room = #-1;
    finally
      valid(item) && item:destroy();
      valid(room) && room:destroy();
    endtry
    return true;
  endverb

  verb test_headless_anonymous_object_identity (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: anonymous fixture creation produces valid anonymous objects while numbered creation does not.";
    anon = $thing:create(true);
    numbered = #-1;
    try
      numbered = $thing:create(0);
      $test_utils:assert_true(valid(anon), "anonymous fixture should be valid");
      $test_utils:assert_true(is_anonymous(anon), "anonymous fixture should report anonymous");
      $test_utils:assert_true(valid(numbered), "numbered fixture should be valid");
      $test_utils:assert_false(is_anonymous(numbered), "numbered fixture should not report anonymous");
      $test_utils:assert_eq(parent(anon), $thing, "anonymous fixture should inherit from $thing");
      $test_utils:assert_eq(parent(numbered), $thing, "numbered fixture should inherit from $thing");
    finally
      valid(numbered) && numbered:destroy();
    endtry
    return true;
  endverb

  verb test_headless_parent_change_invariants (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: changing an object's parent updates the prototype chain and child indexes.";
    child = #-1;
    try
      child = $thing:create();
      old_parent = parent(child);
      chparent(child, $room);
      $test_utils:assert_eq(parent(child), $room, "chparent should update the object's parent");
      $test_utils:assert_true(child in children($room), "chparent should add object to new parent's children");
      $test_utils:assert_false(child in children(old_parent), "chparent should remove object from old parent's children");
    finally
      valid(child) && child:destroy();
    endtry
    return true;
  endverb

  verb test_headless_failed_moveto_preserves_containment (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: a recursive move fails without changing locations or contents.";
    room = #-1;
    box = #-1;
    item = #-1;
    try
      room = $room:create();
      box = $thing:create();
      item = $thing:create();
      box:moveto(room);
      item:moveto(box);
      result = box:moveto(item);
      $test_utils:assert_eq(result, E_RECMOVE, "recursive moveto should report E_RECMOVE");
      $test_utils:assert_eq(box.location, room, "failed moveto should leave source location unchanged");
      $test_utils:assert_eq(item.location, box, "failed moveto should leave nested item location unchanged");
      $test_utils:assert_true(box in room:contents(), "failed moveto should leave source in original container");
      $test_utils:assert_true(item in box:contents(), "failed moveto should leave nested item in source contents");
    finally
      valid(item) && item:destroy();
      valid(box) && box:destroy();
      valid(room) && room:destroy();
    endtry
    return true;
  endverb

  verb test_headless_object_permission_boundary (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: non-owner task permissions cannot mutate another object's protected metadata.";
    target = #-1;
    try
      target = $thing:create();
      target:set_description("unchanged");
      denied = false;
      try
        this:_set_raw_description_as_player(target, "changed");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner metadata mutation should be denied");
      $test_utils:assert_eq(target.description, "unchanged", "denied metadata mutation should preserve description");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_set_description_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not set another object's description through the wizard-owned wrapper.";
    target = #-1;
    try
      target = $thing:create();
      target:set_description("unchanged");
      denied = false;
      try
        this:_call_set_description_as_player(target, "hostile change");
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner set_description wrapper call should be denied");
      $test_utils:assert_eq(target.description, "unchanged", "denied set_description should preserve description");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_set_name_aliases_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not rename another object through the wizard-owned wrapper.";
    target = #-1;
    try
      target = $thing:create();
      target:set_name_aliases("original name", {"original-alias"});
      denied = false;
      try
        this:_call_set_name_aliases_as_player(target, "hostile name", {"hostile-alias"});
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner set_name_aliases wrapper call should be denied");
      $test_utils:assert_eq(target.name, "original name", "denied rename should preserve name");
      $test_utils:assert_eq(target.aliases, {"original-alias"}, "denied rename should preserve aliases");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_set_owner_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not retitle another object through the wizard-owned wrapper.";
    target = #-1;
    try
      target = $thing:create();
      original_owner = target.owner;
      denied = false;
      try
        this:_call_set_owner_as_player(target, player);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner set_owner wrapper call should be denied");
      $test_utils:assert_eq(target.owner, original_owner, "denied owner change should preserve owner");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_moveto_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not move another object through the wizard-owned wrapper.";
    source = #-1;
    old_room = #-1;
    new_room = #-1;
    try
      source = $thing:create();
      old_room = $room:create();
      new_room = $room:create();
      source:moveto(old_room);
      denied = false;
      try
        this:_call_moveto_as_player(source, new_room);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner moveto wrapper call should be denied");
      $test_utils:assert_eq(source.location, old_room, "denied moveto should preserve location");
    finally
      valid(source) && source:destroy();
      valid(new_room) && new_room:destroy();
      valid(old_room) && old_room:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_destroy_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not destroy another object through the wizard-owned wrapper.";
    target = #-1;
    try
      target = $thing:create();
      denied = false;
      try
        this:_call_destroy_as_player(target);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner destroy wrapper call should be denied");
      $test_utils:assert_true(valid(target), "denied destroy should preserve target");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_spoofed_destroy_frame_does_not_authorize_recycle (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-root verb named destroy must not satisfy the recycle interceptor bypass.";
    target = #-1;
    try
      target = $thing:create();
      add_verb(target, {$player, "rxd", "destroy"}, {"this", "none", "this"});
      set_verb_code(target, "destroy", {"recycle(this);", "return true;"});
      denied = false;
      try
        target:destroy();
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-root destroy frame should not authorize direct recycle");
      $test_utils:assert_true(valid(target), "denied spoofed destroy should preserve target");
    finally
      valid(target) && recycle(target);
    endtry
    return true;
  endverb

  verb test_headless_hostile_player_set_home_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: a non-owner helper must not set another player's home through the wizard-owned wrapper.";
    target = #-1;
    new_home = #-1;
    try
      target = create($player);
      new_home = $room:create();
      original_home = target.home;
      denied = false;
      try
        this:_call_set_home_as_player(target, new_home);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner set_home wrapper call should be denied");
      $test_utils:assert_eq(target.home, original_home, "denied set_home should preserve home");
    finally
      valid(target) && target:destroy();
      valid(new_home) && new_home:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_room_dig_check_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: room dig permission checks downgrade before delegation and deny non-owner helpers.";
    target = #-1;
    try
      target = $room:create();
      denied = false;
      try
        this:_call_check_can_dig_from_as_player(target);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner check_can_dig_from call should be denied");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endverb

  verb test_headless_hostile_make_room_in_denied (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Security challenge: area room creation downgrades before delegation and denies non-owner helpers.";
    area = #-1;
    created = #-1;
    try
      area = create($area);
      denied = false;
      try
        created = this:_call_make_room_in_as_player(area, $room);
      except (E_PERM)
        denied = true;
      endtry
      $test_utils:assert_true(denied, "non-owner make_room_in call should be denied");
    finally
      valid(created) && created:destroy();
      valid(area) && area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_wizard_make_room_in_allowed (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: wizard task permissions can create rooms in areas without an explicit grant.";
    area = #-1;
    created = #-1;
    try
      area = create($area);
      created = area:make_room_in($room);
      $test_utils:assert_true(valid(created), "wizard make_room_in should create a valid room");
      $test_utils:assert_eq(created.location, area, "wizard make_room_in should place the room in the area");
    finally
      valid(created) && created:destroy();
      valid(area) && area:destroy();
    endtry
    return true;
  endverb

  verb test_headless_destroy_cleans_containment (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: destroying a container detaches its contents from the recycled location.";
    room = #-1;
    item = #-1;
    try
      room = $room:create();
      item = $thing:create();
      item:moveto(room);
      $test_utils:assert_true(item in room:contents(), "fixture item should start in room contents");
      room:destroy();
      $test_utils:assert_false(valid(room), "destroyed room should be invalid");
      $test_utils:assert_true(valid(item), "destroying room should not recycle contained item");
      $test_utils:assert_eq(item.location, #-1, "destroying room should detach contained item");
      room = #-1;
      item:destroy();
      item = #-1;
    finally
      valid(item) && item:destroy();
      valid(room) && room:destroy();
    endtry
    return true;
  endverb

  verb _set_raw_description_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt protected property mutation under non-owner player task permissions.";
    {target, description} = args;
    target.description = description;
    return true;
  endverb

  verb _call_set_description_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt set_description() through a non-owner player-owned helper.";
    {target, description} = args;
    target:set_description(description);
    return true;
  endverb

  verb _call_set_name_aliases_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt set_name_aliases() through a non-owner player-owned helper.";
    {target, name, aliases} = args;
    target:set_name_aliases(name, aliases);
    return true;
  endverb

  verb _call_set_owner_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt set_owner() through a non-owner player-owned helper.";
    {target, new_owner} = args;
    target:set_owner(new_owner);
    return true;
  endverb

  verb _call_moveto_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt moveto() through a non-owner player-owned helper.";
    {target, destination} = args;
    target:moveto(destination);
    return true;
  endverb

  verb _call_destroy_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt destroy() through a non-owner player-owned helper.";
    {target} = args;
    target:destroy();
    return true;
  endverb

  verb _call_set_home_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt set_home() through a non-owner player-owned helper.";
    {target, home} = args;
    target:set_home(home);
    return true;
  endverb

  verb _call_check_can_dig_from_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt check_can_dig_from() through a non-owner player-owned helper.";
    {target} = args;
    return target:check_can_dig_from();
  endverb

  verb _call_make_room_in_as_player (this none this) owner: PLAYER flags: "rxd"
    "Attempt make_room_in() through a non-owner player-owned helper.";
    {target, room_parent} = args;
    return target:make_room_in(room_parent);
  endverb
endobject
