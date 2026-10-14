object HEADLESS_BOOT_SCENARIOS
  name: "Headless Boot Runtime Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Headless runtime scenarios for bootstrapping a running Cowbell core.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_boot_scenarios";

  verb _assert_valid_global (this none this) owner: HACKER flags: "rxd"
    "Assert that a named sysobj global resolves to the expected object.";
    {prop_name, expected} = args;
    actual = $sysobj.(prop_name);
    $test_utils:assert_type(actual, TYPE_OBJ, "$" + prop_name + " should be an object");
    $test_utils:assert_true(valid(actual), "$" + prop_name + " should be valid");
    $test_utils:assert_eq(actual, expected, "$" + prop_name + " should resolve to expected object");
    return true;
  endverb

  verb test_headless_boot_smoke (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Runtime scenario: the imported core exposes expected globals in a running scheduler-backed MOO.";
    this:_assert_valid_global("sysobj", #0);
    this:_assert_valid_global("root", #1);
    this:_assert_valid_global("player", #5);
    this:_assert_valid_global("room", #7);
    this:_assert_valid_global("event", #18);
    this:_assert_valid_global("sub", #19);
    this:_assert_valid_global("match", #21);
    this:_assert_valid_global("relation", #23);
    this:_assert_valid_global("rule_engine", #62);
    this:_assert_valid_global("test_utils", #127);
    return true;
  endverb
  method test_restart_preserves_authored_room owner: ARCH_WIZARD
    "A restart preserves a builder's description and reaction edits on the First Room.";
    #0:server_started();
    const description = $first_room.description;
    try
      $first_room.description = "A room customized by its owner.";
      let reaction = $reaction:mk('on_enter, 0, {{'announce, "Welcome."}});
      reaction.enabled = false;
      const installed = $reaction:install($first_room, "boot_probe_reaction", reaction);
      #0:server_started();
      $test_utils:assert_eq($first_room.description, "A room customized by its owner.",
        "Startup preserves the authored description");
      $test_utils:assert_true(equal($first_room.boot_probe_reaction, installed),
        "Startup preserves the authored reaction and its disabled state");
      $test_utils:assert_type(installed.authorization, TYPE_STR,
        "The authored reaction retains its signed installation");
    finally
      $first_room.description = description;
      $reaction:remove($first_room, "boot_probe_reaction");
    endtry
    return true;
  endmethod
endobject
