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
  method test_shipped_reactions_use_normal_installation owner: ARCH_WIZARD
    "World initialization installs ordinary grants and a restart preserves later edits.";
    #0:server_started();
    let count = 0;
    for host in ({$couch, $henri, $kibble_cupboard})
      for entry in ($root:_get_reactions_for(host, $arch_wizard, true))
        const {property, reaction} = entry;
        $test_utils:assert_type(reaction.authorization, TYPE_STR, "Shipped reactions need ordinary installation grants");
        $test_utils:assert_eq($reaction:_authority(host, property, reaction), host.owner, "Shipped reactions use the normal installation principal");
        count = count + 1;
      endfor
    endfor
    $test_utils:assert_eq(count, 14, "Every shipped reaction must be installed");
    const original = $henri.cupboard_open_reaction;
    try
      let disabled = original;
      disabled.enabled = false;
      const installed = $reaction:install($henri, "cupboard_open_reaction", disabled);
      #0:server_started();
      $test_utils:assert_true(equal($henri.cupboard_open_reaction, installed), "A restart must not replace or reauthorize edited reactions");
    finally
      $henri.cupboard_open_reaction = original;
    endtry
    return true;
  endmethod
endobject
