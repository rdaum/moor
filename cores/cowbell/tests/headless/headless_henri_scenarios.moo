object HEADLESS_HENRI_SCENARIOS
  name: "Henri Interaction Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true
  override import_export_id = "headless_henri_scenarios";
  override import_export_hierarchy = {"tests", "headless"};

  method _as_guest owner: ARCH_WIZARD
    "Call the real pet or feed operation with an ordinary player's permissions.";
    caller == this || raise(E_PERM);
    const {operation, cat, who, ?food = 0} = args;
    set_task_perms(#90102);
    return operation == 'pet ? $henri:_pet(cat, who) | $henri:_feed(cat, who, food);
  endmethod

  method test_pet_and_feed_boundaries owner: ARCH_WIZARD
    "A forged or remote participant cannot count attempts; feeding checks custody and both policies.";
    const room = create($room, #90100, 2);
    const cat = create($henri, $hacker);
    const who = create($actor, #90102, 2);
    const food = create($cat_kibble, #90102, 2);
    const stone = create($thing, #90102, 2);
    try
      move(cat, room);
      cat:initialize();
      let forged = false;
      try
        this:_as_guest('pet, cat, #90100);
      except (E_PERM)
        forged = true;
      endtry
      $test_utils:assert_true(forged, "Another player cannot be supplied as the actor");
      $test_utils:assert_eq(this:_as_guest('pet, cat, who)['status], 'denied, "A remote attempt is denied");
      $test_utils:assert_eq(cat.pet_attempts, 0, "Unauthenticated and remote attempts do not count");
      move(who, room);
      add_verb(cat, {$hacker, "rxd", "can_be_petted"}, {"this", "none", "this"});
      set_verb_code(cat, "can_be_petted", {"return ['allowed -> true];"});
      $test_utils:assert_eq(this:_as_guest('pet, cat, who)['status], 'denied, "A permissive override cannot bypass the pet rule");
      $test_utils:assert_eq(cat.pet_attempts, 1, "A real refused attempt counts once");
      move(stone, who);
      $test_utils:assert_eq(this:_as_guest('feed, cat, who, stone)['status], 'denied, "The food rule rejects ordinary things");
      $test_utils:assert_eq(this:_as_guest('feed, cat, who, food)['status], 'denied, "Food must be carried");
      move(food, who);
      food.drop_rule = $rule_engine:parse_expression("FALSE", 'keep, $rule_engine:policy_scope('drop_rule));
      $test_utils:assert_eq(this:_as_guest('feed, cat, who, food)['status], 'denied, "Feeding respects the item's drop rule");
      $test_utils:assert_eq({food.location, cat.pet_attempts, cat.mood}, {who, 1, "grouchy"}, "Rejected feeding changes neither custody nor cat state");
      food.drop_rule = 0;
      $test_utils:assert_eq(this:_as_guest('feed, cat, who, food)['status], 'applied, "Carried kibble is accepted");
      $test_utils:assert_eq({food.location, cat.pet_attempts, cat.mood}, {cat, 0, "sleepy"}, "The completed feeding reaction owns the response");
    finally
      recycle(stone);
      recycle(food);
      recycle(who);
      recycle(cat);
      recycle(room);
    endtry
    return true;
  endmethod

  method test_feeding_rechecks_after_acceptance owner: ARCH_WIZARD
    "A destination callback cannot authorize a transfer after changing the cat's food policy.";
    const room = create($room, #90100, 2);
    const cat = create($henri, $hacker);
    const who = create($actor, #90102, 2);
    const food = create($cat_kibble, #90102, 2);
    try
      move(cat, room);
      move(who, room);
      move(food, who);
      cat:initialize();
      add_verb(cat, {$hacker, "rxd", "acceptable"}, {"this", "none", "this"});
      set_verb_code(cat, "acceptable", {
        "this.feed_rule = 0;",
        "suspend(0);",
        "return true;"
      });
      const result = this:_as_guest('feed, cat, who, food);
      $test_utils:assert_eq(result['status], 'denied, "Changed policy prevents a completed operation");
      $test_utils:assert_eq({food.location, cat.mood}, {who, "grouchy"}, "No transfer or feeding reaction follows a changed policy");
    finally
      recycle(food);
      recycle(who);
      recycle(cat);
      recycle(room);
    endtry
    return true;
  endmethod
endobject
