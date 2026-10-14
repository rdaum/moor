object HEADLESS_SCHEDULER_SCENARIOS
  name: "Headless Native Schedule Scenarios"
  parent: ROOT
  owner: HACKER
  readable: true

  override description = "Native schedule lifecycle scenarios for housekeeping.";
  override import_export_hierarchy = {"tests", "headless"};
  override import_export_id = "headless_scheduler_scenarios";

  method test_headless_housekeeping_native_lifecycle owner: ARCH_WIZARD
    "Housekeeping starts once, remains recurring after empty sweeps, and cancels future firings.";
    const housekeeping = create($housekeeping, $arch_wizard, 2);
    try
      housekeeping.schedule_id = 0;
      housekeeping.sweep_interval = 0.1;
      housekeeping:start();
      const id = housekeeping.schedule_id;
      housekeeping:start();
      $test_utils:assert_eq(housekeeping.schedule_id, id, "duplicate start must retain one schedule");
      const deadline = time() + 5;
      "Commit native registration before querying runtime diagnostics.";
      suspend(0);
      let info = schedule_info(id);
      while (time() <= deadline && info["run_count"] == 0)
        "Commit registration and wait for an actual housekeeping sweep.";
        suspend(0.1);
        info = schedule_info(id);
      endwhile
      $test_utils:assert_true(info["run_count"] > 0, "native sweep must execute");
      $test_utils:assert_eq(info["target"], housekeeping, "schedule targets the fixture");
      $test_utils:assert_false(info["adaptive"], "sweep count must not change the recurrence");
      $test_utils:assert_false(info["pass_elapsed"], "sweep argument contract remains unchanged");
      $test_utils:assert_true(schedule_valid(id), "an empty sweep must not retire the recurrence");
      housekeeping:stop();
      "Commit cancellation before checking the runtime schedule store.";
      suspend(0);
      $test_utils:assert_false(schedule_valid(id), "stop must cancel future sweeps");
      $test_utils:assert_eq(housekeeping.schedule_id, 0, "stop clears the stored ID");
    finally
      valid(housekeeping) && housekeeping:stop();
      valid(housekeeping) && housekeeping:destroy();
    endtry
    return true;
  endmethod

  method test_headless_housekeeping_stale_id owner: ARCH_WIZARD
    "An objdef-style stale ID does not prevent a new native housekeeping schedule.";
    const housekeeping = create($housekeeping, $arch_wizard, 2);
    try
      housekeeping.schedule_id = 999999;
      housekeeping:start();
      $test_utils:assert_true(schedule_valid(housekeeping.schedule_id), "start replaces a stale ID");
      $test_utils:assert_true(housekeeping.schedule_id != 999999, "the stale ID must be replaced");
      housekeeping:stop();
      housekeeping:stop();
    finally
      valid(housekeeping) && housekeeping:stop();
      valid(housekeeping) && housekeeping:destroy();
    endtry
    return true;
  endmethod

  method test_headless_native_app_controls owner: ARCH_WIZARD
    "Untrusted callers cannot start or stop housekeeping.";
    const housekeeping = create($housekeeping, $arch_wizard, 2);
    const recording_player = this:_recording_player();
    player = recording_player;
    try
      for operation in ({"housekeeping_start", "housekeeping_stop"})
        let denied = false;
        try
          this:_native_app_call_as_player(housekeeping, operation);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, operation + " must reject the foreign principal");
      endfor
    finally
      valid(housekeeping) && housekeeping:stop();
      valid(housekeeping) && housekeeping:destroy();
      valid(recording_player) && recording_player:destroy();
    endtry
    return true;
  endmethod

  method _native_app_call_as_player owner: PLAYER
    "Call only application control regression methods as the player principal.";
    caller == this && this == #90001 || raise(E_PERM);
    const {housekeeping, operation} = args;
    operation == "housekeeping_start" && return housekeeping:start();
    operation == "housekeeping_stop" && return housekeeping:stop();
    raise(E_PERM);
  endmethod
  method _recording_player owner: ARCH_WIZARD
    "Create a test-only player that records current output without a network connection.";
    caller == this && this == #90001 || raise(E_PERM);
    const recipient = create($player, $arch_wizard, 2);
    add_property(recipient, "recorded_events", {}, {$arch_wizard, "r"});
    add_verb(recipient, {$arch_wizard, "rxd", "inform_current"}, {"this", "none", "this"});
    set_verb_code(recipient, "inform_current", {
      "const {event} = args;",
      "this.recorded_events = {@this.recorded_events, event};",
      "return true;"});
    return recipient;
  endmethod

  method test_headless_housekeeping_grouped_sweep owner: ARCH_WIZARD
    "Two disconnected players from one room produce two moves and one grouped announcement.";
    const housekeeping = create($housekeeping, $arch_wizard, 2);
    const room = create($room, $arch_wizard, 2);
    const home = create($room, $arch_wizard, 2);
    const first = this:_recording_player();
    const second = this:_recording_player();
    try
      add_property(room, "housekeeper", housekeeping, {$arch_wizard, "r"});
      add_property(room, "housekeeping_events", {}, {$arch_wizard, "r"});
      add_verb(room, {$arch_wizard, "rxd", "announce"}, {"this", "none", "this"});
      set_verb_code(room, "announce", {
        "const {event} = args;",
        "if (event.actor == this.housekeeper)",
        "  this.housekeeping_events = {@this.housekeeping_events, event};",
        "endif",
        "return true;"});
      housekeeping.idle_threshold = 1;
      for participant in ({first, second})
        set_player_flag(participant, 1);
        participant.home = home;
        participant.last_disconnected = time() - 10;
        add_verb(participant, {$arch_wizard, "rxd", "moveto"}, {"this", "none", "this"});
        set_verb_code(participant, "moveto", {"const {destination} = args;", "move(this, destination);", "return true;"});
        add_verb(participant, {$arch_wizard, "rxd", "tell"}, {"this", "none", "this"});
        set_verb_code(participant, "tell", {"return true;"});
        move(participant, room);
      endfor
      $test_utils:assert_eq(housekeeping:sweep(), 2, "both successful moves must remain in the accumulator");
      $test_utils:assert_eq(first.location, home, "first player must move home");
      $test_utils:assert_eq(second.location, home, "second player must move home");
      $test_utils:assert_eq(length(room.housekeeping_events), 1, "one room must receive one grouped announcement");
    finally
      for participant in ({first, second})
        valid(participant) && set_player_flag(participant, 0);
        valid(participant) && participant:destroy();
      endfor
      valid(room) && room:destroy();
      valid(home) && home:destroy();
      valid(housekeeping) && housekeeping:destroy();
    endtry
    return true;
  endmethod

  method _record_command_player owner: ARCH_WIZARD
    "Record output on the actual task player used by dispatch_command_verb().";
    caller == this && this == #90001 && player == #90100 || raise(E_PERM);
    const roles = {player.wizard, player.programmer};
    add_property(player, "recorded_events", {}, {$arch_wizard, "r"});
    add_verb(player, {$arch_wizard, "rxd", "inform_current"}, {"this", "none", "this"});
    set_verb_code(player, "inform_current", {
      "const {event} = args;",
      "this.recorded_events = {@this.recorded_events, event};",
      "return true;"});
    player.wizard = false;
    player.programmer = true;
    return {player, roles};
  endmethod

  method _restore_command_player owner: ARCH_WIZARD
    "Restore the fixture player's roles and inherited notification method.";
    caller == this && this == #90001 || raise(E_PERM);
    const {actor, roles} = args;
    actor == #90100 || raise(E_PERM);
    actor.wizard = roles[1];
    actor.programmer = roles[2];
    delete_verb(actor, "inform_current");
    delete_property(actor, "recorded_events");
  endmethod

  method _schedule_command owner: ARCH_WIZARD
    "Dispatch a schedule command as a fixture actor and return its rendered output.";
    caller == this && this == #90001 || raise(E_PERM);
    const {actor, name, ?text = ""} = args;
    actor == player || raise(E_INVARG);
    actor.recorded_events = {};
    const command = parse_command(name + (text ? " " + text | ""), {});
    dispatch_command_verb($prog_features, name, command);
    return toliteral($document:render(actor.recorded_events[$]:resolve(actor), 'text_plain)['body]);
  endmethod

  method _schedule_rows_as owner: ARCH_WIZARD
    "Read schedule rows with the fixture actor's permissions, regardless of the task player.";
    caller == this && this == #90001 || raise(E_PERM);
    const {actor} = args;
    set_task_perms(actor);
    return $prog_features:_schedule_rows();
  endmethod

  method _create_command_schedule owner: ARCH_WIZARD
    "Create a long-lived fixture schedule owned by the selected principal.";
    caller == this && this == #90001 || raise(E_PERM);
    const {actor, ?kind = "every", ?interval = 3600.0} = args;
    set_task_perms(actor);
    const options = ['player -> actor, 'pass_elapsed -> false, 'adaptive -> kind == "adaptive"];
    if (kind == "at")
      return schedule_at(this, "_schedule_command_firing", time() + 3600, {}, options);
    endif
    return schedule_every(this, "_schedule_command_firing", interval, {}, options);
  endmethod

  method _schedule_command_firing owner: ARCH_WIZARD
    "Retain a test firing until its task is explicitly killed.";
    "The suspension commits startup so another task can inspect the running firing.";
    suspend(300);
    return 3600.0;
  endmethod

  method test_headless_schedule_command_listing owner: ARCH_WIZARD
    "List one-shot, recurring, and adaptive schedules with owner filtering in both commands.";
    const {owner, roles} = this:_record_command_player();
    const other = this:_recording_player();
    owner.programmer = true;
    other.programmer = true;
    let ids = {};
    try
      for kind in ({"at", "every", "adaptive"})
        ids = {@ids, this:_create_command_schedule(owner, kind)};
      endfor
      const foreign = this:_create_command_schedule(other);
      ids = {@ids, foreign};
      "Commit registrations before reading runtime diagnostics.";
      suspend(0);
      const rows = this:_schedule_rows_as(owner);
      $test_utils:assert_eq({row[1] for row in (rows)}, {tostr(id) for id in (ids[1..3])},
        "ordinary programmers see only their schedules");
      $test_utils:assert_eq(rows[1][4], "once", "one-shot timing is distinct");
      $test_utils:assert_true(index(rows[2][4], "every"), "fixed recurrence is visible");
      $test_utils:assert_true(index(rows[3][4], "adaptive"), "adaptive recurrence is visible");
      $test_utils:assert_true(index(rows[1][5], "in "), "deadline is visible");
      $test_utils:assert_eq(rows[1][6], "-", "idle schedule has no task");
      for name in ({"@ps", "@tasks", "@schedules"})
        const output = this:_schedule_command(owner, name);
        $test_utils:assert_true(index(output, "3 scheduled"), name + " includes the schedule count");
        $test_utils:assert_true(index(output, "Schedule ID"), name + " labels schedule IDs");
        $test_utils:assert_true(index(output, "Running task"), name + " labels firing task IDs");
      endfor
      owner.wizard = true;
      const all_ids = {visible_row[1] for visible_row in (this:_schedule_rows_as(owner))};
      $test_utils:assert_true(tostr(foreign) in all_ids, "wizards see other owners' schedules");
    finally
      for id in (ids)
        schedule_stop(id);
      endfor
      this:_restore_command_player(owner, roles);
      other:destroy();
    endtry
    return true;
  endmethod

  method test_headless_schedule_command_management owner: ARCH_WIZARD
    "Inspect and stop schedules with owner checks, wizard access, and clear invalid-ID results.";
    const {owner, roles} = this:_record_command_player();
    const other = this:_recording_player();
    owner.programmer = true;
    other.programmer = true;
    let id = 0;
    let foreign = 0;
    try
      id = this:_create_command_schedule(owner);
      foreign = this:_create_command_schedule(other);
      "Commit registrations before command inspection.";
      suspend(0);
      const output = this:_schedule_command(owner, "@schedule", tostr(id));
      $test_utils:assert_true(index(output, "_schedule_command_firing"), "details include the callback");
      $test_utils:assert_true(index(output, "Runs / faults"), "details include diagnostics");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@schedule", tostr(foreign)),
        "Permission denied"), "foreign diagnostics remain private");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@stop-schedule", tostr(foreign)),
        "Permission denied"), "foreign cancellation is denied");
      for invalid in ({"", "0", "-1", "1.5", "1junk", "1 2", "9999999999999999999999999999999"})
        for name in ({"@schedule", "@stop-schedule"})
          $test_utils:assert_true(index(this:_schedule_command(owner, name, invalid), "Usage:"),
            "malformed IDs must not select another schedule");
        endfor
      endfor
      owner.programmer = false;
      const denied = `this:_schedule_command(owner, "@stop-schedule", tostr(id)) ! E_PERM => E_PERM';
      $test_utils:assert_eq(denied, E_PERM, "feature access does not grant programmer authority");
      owner.programmer = true;
      $test_utils:assert_true(schedule_valid(id) && schedule_valid(foreign), "denials leave schedules live");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@stop-schedule", tostr(id)),
        "Stopped schedule"), "owner may stop recurrence");
      "Commit the stop before checking runtime state.";
      suspend(0);
      $test_utils:assert_false(schedule_valid(id), "owner cancellation reaches the scheduler");
      $test_utils:assert_true(schedule_valid(foreign), "foreign schedule remains live");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@stop-schedule", tostr(id)),
        "No live schedule"), "repeated stops are harmless");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@schedule", tostr(id)),
        "No such schedule"), "removed IDs have a useful diagnostic");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@schedules"), "(none)"),
        "empty schedule list is explicit");
      owner.wizard = true;
      $test_utils:assert_true(index(this:_schedule_command(owner, "@schedule", tostr(foreign)),
        "_schedule_command_firing"), "wizard may inspect another owner's schedule");
      $test_utils:assert_true(index(this:_schedule_command(owner, "@kill-schedule", tostr(foreign)),
        "Stopped schedule"), "wizard may use the stop alias for another owner");
      "Commit wizard cancellation before checking runtime state.";
      suspend(0);
      $test_utils:assert_false(schedule_valid(foreign), "wizard cancellation reaches the scheduler");
    finally
      schedule_stop(id);
      schedule_stop(foreign);
      this:_restore_command_player(owner, roles);
      other:destroy();
    endtry
    return true;
  endmethod

  method test_headless_schedule_stop_preserves_firing owner: ARCH_WIZARD
    "Stopping recurrence leaves its current task visible and available to @kill.";
    const {actor, roles} = this:_record_command_player();
    actor.programmer = true;
    let id = 0;
    let firing = 0;
    try
      id = this:_create_command_schedule(actor, "every", 0.1);
      const deadline = time() + 5;
      "Commit registration, then wait for the actual firing to start.";
      suspend(0);
      while (!firing && time() <= deadline)
        firing = schedule_info(id)["running_task"];
        !firing && suspend(0.05);
      endwhile
      $test_utils:assert_true(firing, "recurring schedule must dispatch a task");
      const rows = this:_schedule_rows_as(actor);
      $test_utils:assert_eq(rows[1][6], tostr(firing), "schedule listing links to its running task");
      const output = this:_schedule_command(actor, "@stop-schedule", tostr(id));
      $test_utils:assert_true(index(output, "Running firings are unchanged"), "stop explains task behavior");
      "Commit cancellation before checking the surviving firing.";
      suspend(0);
      $test_utils:assert_false(schedule_valid(id), "future firings are stopped");
      $test_utils:assert_true(valid_task(firing), "current firing survives recurrence cancellation");
      "The fixture callback is wizard-owned; killing its task requires wizard authority.";
      actor.wizard = true;
      $test_utils:assert_true(index(this:_schedule_command(actor, "@kill", tostr(firing)),
        "Killed task"), "existing task command controls the firing");
    finally
      schedule_stop(id);
      firing && `kill_task(firing) ! E_INVARG';
      this:_restore_command_player(actor, roles);
    endtry
    return true;
  endmethod

endobject
