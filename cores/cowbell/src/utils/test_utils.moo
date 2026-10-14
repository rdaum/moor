object TEST_UTILS [
  import_export_id -> "test_utils",
  import_export_hierarchy -> {"utils"}
]
  name: "Test Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Shared helpers for cowbell unit tests.";

  method assert_true owner: HACKER
    "Assert that condition is true.";
    {condition, ?message = "Expected true"} = args;
    condition || raise(E_ASSERT, message);
    return true;
  endmethod

  method assert_false owner: HACKER
    "Assert that condition is false.";
    {condition, ?message = "Expected false"} = args;
    !condition || raise(E_ASSERT, message);
    return true;
  endmethod

  method assert_eq owner: HACKER
    "Assert that actual equals expected.";
    {actual, expected, ?message = "Values differ"} = args;
    actual == expected || raise(E_ASSERT, message + ": expected " + toliteral(expected) + ", got " + toliteral(actual));
    return true;
  endmethod

  method assert_type owner: HACKER
    "Assert that value has the expected MOO type constant.";
    {value, expected_type, ?message = "Type mismatch"} = args;
    typeof(value) == expected_type || raise(E_ASSERT, message + ": expected type " + tostr(expected_type) + ", got " + tostr(typeof(value)));
    return true;
  endmethod

  method assert_raises owner: HACKER
    "Assert that obj:verb(@call_args) raises expected_error.";
    {expected_error, obj, verb_name, ?call_args = {}, ?message = "Expected error"} = args;
    try
      obj:(verb_name)(@call_args);
    except e (ANY)
      e[1] == expected_error || raise(E_ASSERT, message + ": expected " + tostr(expected_error) + ", got " + tostr(e[1]));
      return true;
    endtry
    raise(E_ASSERT, message + ": no error raised");
  endmethod

  method anonymous owner: HACKER
    "Create an anonymous child of proto for test fixtures.";
    {proto} = args;
    return proto:create(true);
  endmethod

  method destroy_if_valid owner: HACKER
    "Destroy a persistent fixture if it is valid. Anonymous objects cannot be destroyed.";
    {obj} = args;
    if (valid(obj) && !is_anonymous(obj))
      obj:destroy();
    endif
    return true;
  endmethod

  method test_assertions owner: HACKER
    "Exercise the basic assertion helpers.";
    this:assert_true(true, "true should pass");
    this:assert_false(false, "false should pass");
    this:assert_eq("cow", "cow", "equal strings should pass");
    this:assert_type("cow", TYPE_STR, "string should have TYPE_STR");
    this:assert_raises(E_ASSERT, this, "assert_eq", {"cow", "bell", "mismatch should raise"}, "assert_eq mismatch");
    this:assert_eq($arch_wizard:pronouns_display(), "they/them", "player pronouns display should be readable");
    const actor = this:anonymous($actor);
    actor.pronouns = $pronouns:lookup("he/him");
    this:assert_eq(actor:pronouns_display(), "he/him", "actor pronouns display should be readable");
    return true;
  endmethod

  method test_anonymous_fixture_helpers owner: HACKER
    "Exercise anonymous creation and safe cleanup helpers.";
    anon = this:anonymous($root);
    this:assert_true(is_anonymous(anon), "anonymous helper should create anonymous objects");
    this:destroy_if_valid(anon);
    this:assert_true(valid(anon), "destroy_if_valid should leave anonymous objects alone");
    scratch = create($root);
    try
      this:assert_false(is_anonymous(scratch), "default create should make a persistent object");
      this:destroy_if_valid(scratch);
      this:assert_false(valid(scratch), "destroy_if_valid should destroy persistent objects");
    finally
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod
endobject
