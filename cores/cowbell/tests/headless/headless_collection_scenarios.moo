// Copyright (C) 2026 The mooR Authors
// SPDX-License-Identifier: GPL-3.0-or-later
object HEADLESS_COLLECTION_SCENARIOS
  name: "Headless Collection Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method _as_hacker owner: HACKER
    "Call a collection method as a non-wizard programmer.";
    const {target, name, @arguments} = args;
    return target:(name)(@arguments);
  endmethod

  method test_queue_permissions_and_order owner: ARCH_WIZARD
    "Owners can use their queues. Foreign callers cannot bypass property permissions.";
    const own = create($agentic.event_queue, $hacker);
    const foreign = create($agentic.event_queue, $arch_wizard);
    try
      foreign.queue = {"private"};
      set_property_info(foreign, "queue", {$arch_wizard, ""});
      for operation in ({{"push", "injected"}, {"pop"}, {"clear"}, {"size"}})
        $test_utils:assert_raises(E_PERM, this, "_as_hacker", {foreign, @operation},
          "Foreign queue operation must fail");
      endfor
      $test_utils:assert_eq(foreign.queue, {"private"}, "Denied calls preserve the queue");
      $test_utils:assert_eq(this:_as_hacker(own, "push", 0), 1, "First append");
      $test_utils:assert_eq(this:_as_hacker(own, "push", "second"), 2, "Second append");
      $test_utils:assert_eq(this:_as_hacker(own, "pop"), 0, "Zero is a valid entry");
      $test_utils:assert_eq(this:_as_hacker(own, "size"), 1, "Pop removes one entry");
      $test_utils:assert_eq(this:_as_hacker(own, "pop"), "second", "FIFO order");
      $test_utils:assert_eq(this:_as_hacker(own, "pop"), 0, "Empty queue sentinel");
      this:_as_hacker(own, "push", "discard");
      $test_utils:assert_eq(this:_as_hacker(own, "clear"), 1, "Clear result");
      $test_utils:assert_eq(this:_as_hacker(own, "size"), 0, "Clear removes all entries");
    finally
      recycle(own);
      recycle(foreign);
    endtry
    return true;
  endmethod

  method test_message_bag_permissions_and_values owner: ARCH_WIZARD
    "Object bags enforce property permissions. Flyweight updates preserve the original.";
    const own = create($msg_bag, $hacker);
    const foreign = create($msg_bag, $arch_wizard);
    try
      foreign.entries = {"private"};
      set_property_info(foreign, "entries", {$arch_wizard, ""});
      for operation in ({{"add", "injected"}, {"remove", 1}, {"set_entry", 1, "changed"},
          {"list"}, {"entries"}, {"pick"}})
        $test_utils:assert_raises(E_PERM, this, "_as_hacker", {foreign, @operation},
          "Foreign bag operation must fail");
      endfor
      $test_utils:assert_eq(foreign.entries, {"private"}, "Denied calls preserve the bag");
      this:_as_hacker(own, "add", "first");
      this:_as_hacker(own, "set_entry", 1, "changed");
      $test_utils:assert_eq(this:_as_hacker(own, "list"), {"changed"}, "List alias");
      $test_utils:assert_eq(this:_as_hacker(own, "entries"), {"changed"}, "Entries alias");
      this:_as_hacker(own, "remove", 1);
      $test_utils:assert_eq(this:_as_hacker(own, "pick"), E_RANGE, "Empty bag sentinel");

      const original = $msg_bag:mk("first");
      const added = original:add("second");
      const replaced = added:set_entry(1, "changed");
      const removed = replaced:remove(2);
      $test_utils:assert_eq(original:list(), {"first"}, "Original bag is unchanged");
      $test_utils:assert_eq(added:entries(), {"first", "second"}, "Add creates a new bag");
      $test_utils:assert_eq(removed:pick(), "changed", "Replace and remove compose");
      for bag in ({own, original})
        $test_utils:assert_raises(E_RANGE, bag, "remove", {0}, "Reject index zero");
        $test_utils:assert_raises(E_RANGE, bag, "set_entry", {2, "bad"}, "Reject past end");
        $test_utils:assert_raises(E_TYPE, bag, "remove", {"1"}, "Require integer index");
      endfor
    finally
      recycle(own);
      recycle(foreign);
    endtry
    return true;
  endmethod
endobject
