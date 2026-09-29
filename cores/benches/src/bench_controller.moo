object BENCH_CONTROLLER [
  import_export_id -> "bench_controller"
]
  name: "Game Update Benchmark Controller"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property cycles_per_sample (owner: ARCH_WIZARD, flags: "rw") = 20;
  property running (owner: ARCH_WIZARD, flags: "r") = 0;
  property samples_per_level (owner: ARCH_WIZARD, flags: "rw") = 10;
  property subscriber_counts (owner: ARCH_WIZARD, flags: "rw") = {1, 4, 16, 64, 256, 512, 1024};
  property subscribers (owner: ARCH_WIZARD, flags: "rw") = {};
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;

  method run owner: ARCH_WIZARD
    ":run() => NONE";
    "  Runs the full benchmark suite, logging results for histogram plotting.";
    if (this.running)
      player:tell("Benchmark already running.");
      return;
    endif
    this.running = 1;
    server_log("=== GAME_UPDATE BENCHMARK START ===");
    server_log("cycles_per_sample=" + tostr(this.cycles_per_sample) + " samples_per_level=" + tostr(this.samples_per_level) + " work_iterations=" + tostr(this.work_iterations));
    try
      for target_count in (this.subscriber_counts)
        this:set_subscriber_count(target_count);
        this:run_samples(target_count);
      endfor
    finally
      this:cleanup();
      this.running = 0;
      server_log("=== GAME_UPDATE BENCHMARK END ===");
    endtry
  endmethod

  method set_subscriber_count owner: ARCH_WIZARD
    {target} = args;
    current = length(this.subscribers);
    if (current < target)
      "Add subscribers";
      for i in [1..target - current]
        sub = create(#667, #6);
        sub.work_iterations = this.work_iterations;
        this.subscribers = {@this.subscribers, sub};
        #666:register(sub);
      endfor
    elseif (current > target)
      "Remove subscribers";
      to_remove = current - target;
      for i in [1..to_remove]
        sub = this.subscribers[$];
        #666:unregister(sub);
        recycle(sub);
        this.subscribers = this.subscribers[1..$ - 1];
      endfor
    endif
    commit();
    "Let things settle";
    suspend(0.5);
  endmethod

  method run_samples owner: ARCH_WIZARD
    {subscriber_count} = args;
    update_hz = #666.update_hertz;
    cycles = this.cycles_per_sample;
    wait_time = cycles / update_hz;
    for sample_num in [1..this.samples_per_level]
      "Wait for cycles to complete";
      suspend(wait_time);
      "Sample latency - take mean of available samples";
      latencies = #666.latency;
      if (length(latencies) == 0)
        continue;
      endif
      sum = 0.0;
      for lat in (latencies)
        sum = sum + lat;
      endfor
      mean_latency = sum / length(latencies);
      min_latency = latencies[1];
      max_latency = latencies[1];
      for lat in (latencies)
        min_latency = min(min_latency, lat);
        max_latency = max(max_latency, lat);
      endfor
      "Log in CSV-ish format for easy parsing";
      server_log("BENCH_DATA subscribers=" + tostr(subscriber_count) + " sample=" + tostr(sample_num) + " mean_ms=" + tostr(mean_latency * 1000.0) + " min_ms=" + tostr(min_latency * 1000.0) + " max_ms=" + tostr(max_latency * 1000.0));
    endfor
  endmethod

  method cleanup owner: ARCH_WIZARD
    ":cleanup() => NONE";
    "  Removes and recycles all benchmark subscribers.";
    for sub in (this.subscribers)
      if (valid(sub))
        #666:unregister(sub);
        recycle(sub);
      endif
    endfor
    this.subscribers = [];
    #666:clear_faults();
    server_log("Benchmark cleanup complete. All subscribers removed.");
  endmethod

  method capture_perf_counters owner: ARCH_WIZARD
    ":capture_perf_counters() => MAP";
    snapshot = [];
    snapshot["bf"] = bf_counters();
    snapshot["db"] = db_counters();
    snapshot["sched"] = sched_counters();
    return snapshot;
  endmethod

  method capture_property_cache_stats owner: ARCH_WIZARD
    ":capture_property_cache_stats() => LIST";
    return property_cache_stats();
  endmethod

  method capture_verb_cache_stats owner: ARCH_WIZARD
    ":capture_verb_cache_stats() => LIST";
    return verb_cache_stats();
  endmethod

  method log_property_cache_delta owner: ARCH_WIZARD
    ":log_property_cache_delta(STR label, LIST before, LIST after) => NONE";
    {label, before, after} = args;
    hits = `after[1] ! E_RANGE => 0' - `before[1] ! E_RANGE => 0';
    negative_hits = `after[2] ! E_RANGE => 0' - `before[2] ! E_RANGE => 0';
    misses = `after[3] ! E_RANGE => 0' - `before[3] ! E_RANGE => 0';
    flushes = `after[4] ! E_RANGE => 0' - `before[4] ! E_RANGE => 0';
    after_hist = `after[5] ! E_RANGE => {0, 0}';
    before_hist = `before[5] ! E_RANGE => {0, 0}';
    entries = `after_hist[2] ! E_RANGE => 0';
    entries_delta = entries - `before_hist[2] ! E_RANGE => 0';
    total = hits + negative_hits + misses;
    hit_rate = total > 0 ? tofloat(hits + negative_hits) * 100.0 / tofloat(total) | 0.0;
    server_log("CACHE_SUM label=" + label + ":prop_cache hits=" + tostr(hits) + " negative_hits=" + tostr(negative_hits) + " misses=" + tostr(misses) + " flushes=" + tostr(flushes) + " entries=" + tostr(entries) + " entries_delta=" + tostr(entries_delta) + " hit_rate=" + tostr(hit_rate));
  endmethod

  method log_verb_cache_delta owner: ARCH_WIZARD
    ":log_verb_cache_delta(STR label, LIST before, LIST after) => NONE";
    {label, before, after} = args;
    hits = `after[1] ! E_RANGE => 0' - `before[1] ! E_RANGE => 0';
    negative_hits = `after[2] ! E_RANGE => 0' - `before[2] ! E_RANGE => 0';
    misses = `after[3] ! E_RANGE => 0' - `before[3] ! E_RANGE => 0';
    flushes = `after[4] ! E_RANGE => 0' - `before[4] ! E_RANGE => 0';
    after_hist = `after[5] ! E_RANGE => {0, 0}';
    before_hist = `before[5] ! E_RANGE => {0, 0}';
    entries = `after_hist[2] ! E_RANGE => 0';
    entries_delta = entries - `before_hist[2] ! E_RANGE => 0';
    total = hits + negative_hits + misses;
    hit_rate = total > 0 ? tofloat(hits + negative_hits) * 100.0 / tofloat(total) | 0.0;
    server_log("CACHE_SUM label=" + label + ":verb_cache hits=" + tostr(hits) + " negative_hits=" + tostr(negative_hits) + " misses=" + tostr(misses) + " flushes=" + tostr(flushes) + " entries=" + tostr(entries) + " entries_delta=" + tostr(entries_delta) + " hit_rate=" + tostr(hit_rate));
  endmethod

  method counter_delta owner: ARCH_WIZARD
    ":counter_delta(MAP before, MAP after) => MAP";
    {before, after} = args;
    delta = [];
    for after_vals, op in (after)
      before_vals = `before[op] ! E_RANGE => {0, 0}';
      calls = after_vals[1] - before_vals[1];
      nanos = after_vals[2] - before_vals[2];
      if (calls > 0 || nanos > 0)
        delta[op] = {calls, nanos};
      endif
    endfor
    return delta;
  endmethod

  method log_top_counter_deltas owner: ARCH_WIZARD
    ":log_top_counter_deltas(STR label, MAP delta, INT limit) => NONE";
    {label, delta, ?limit = 10} = args;
    ops = mapkeys(delta);
    if (!ops)
      server_log("PERF_TOP label=" + label + " rank=0 op=none calls=0 nanos=0 avg_ns=0");
      return;
    endif
    max_to_log = min(max(1, limit), length(ops));
    for rank in [1..max_to_log]
      best = ops[1];
      best_vals = delta[best];
      for op in (ops)
        vals = delta[op];
        if (vals[2] > best_vals[2])
          best = op;
          best_vals = vals;
        endif
      endfor
      calls = best_vals[1];
      nanos = best_vals[2];
      avg_ns = calls > 0 ? toint(nanos / calls) | 0;
      server_log("PERF_TOP label=" + label + " rank=" + tostr(rank) + " op=" + tostr(best) + " calls=" + tostr(calls) + " nanos=" + tostr(nanos) + " avg_ns=" + tostr(avg_ns));
      ops = setremove(ops, best);
      if (!ops)
        return;
      endif
    endfor
  endmethod

  method find_counter_delta owner: ARCH_WIZARD
    ":find_counter_delta(MAP delta, STR op_name) => LIST";
    {delta, op_name} = args;
    for vals, op in (delta)
      if (tostr(op) == op_name)
        return vals;
      endif
    endfor
    return {0, 0};
  endmethod

  method log_key_counter_deltas owner: ARCH_WIZARD
    ":log_key_counter_deltas(STR label, MAP delta, LIST op_names) => NONE";
    {label, delta, op_names} = args;
    for op_name in (op_names)
      vals = this:find_counter_delta(delta, op_name);
      calls = vals[1];
      nanos = vals[2];
      avg_ns = calls > 0 ? toint(nanos / calls) | 0;
      server_log("PERF_KEY label=" + label + " op=" + op_name + " calls=" + tostr(calls) + " nanos=" + tostr(nanos) + " avg_ns=" + tostr(avg_ns));
    endfor
  endmethod

  method log_perf_delta owner: ARCH_WIZARD
    ":log_perf_delta(STR label, MAP before_snapshot, MAP after_snapshot) => NONE";
    {label, before_snapshot, after_snapshot} = args;
    all_delta = [];
    for category in ({"sched", "db", "bf"})
      before_cat = `before_snapshot[category] ! E_RANGE => []';
      after_cat = `after_snapshot[category] ! E_RANGE => []';
      delta = this:counter_delta(before_cat, after_cat);
      for vals, op in (delta)
        all_delta[category + "." + tostr(op)] = vals;
      endfor
      total_calls = 0;
      total_nanos = 0;
      for vals, op in (delta)
        total_calls = total_calls + vals[1];
        total_nanos = total_nanos + vals[2];
      endfor
      avg_ns = total_calls > 0 ? toint(total_nanos / total_calls) | 0;
      category_label = label + ":" + category;
      server_log("PERF_SUM label=" + category_label + " calls=" + tostr(total_calls) + " nanos=" + tostr(total_nanos) + " avg_ns=" + tostr(avg_ns));
      this:log_top_counter_deltas(category_label, delta, 8);
      if (category == "sched")
        this:log_key_counter_deltas(category_label, delta, {"task_recv_immediate_resume_latency", "task_message_delivery_to_recv_latency", "task_wake_signal_to_dispatch_start_latency", "task_wake_to_dispatch_latency"});
      elseif (category == "db")
        this:log_key_counter_deltas(category_label, delta, {"commit_check_phase", "commit_apply_phase", "commit_process_phase", "commit_wait_phase", "apply_index_insert", "batch_writer_backpressure", "batch_writer_backpressure_block", "provider_pending_ops_read_lock_wait", "provider_pending_ops_write_lock_wait"});
        this:log_key_counter_deltas(category_label, delta, {"property_list_append_classify", "property_list_append_candidate", "property_list_append_accepted", "property_list_append_suffix_elements", "property_list_append_suffix_bytes", "property_list_append_missing_base", "property_list_append_non_list", "property_list_append_not_longer", "property_list_append_prefix_mismatch", "property_list_append_comparison_budget", "property_value_complete_replacement"});
        this:log_key_counter_deltas(category_label, delta, {"property_value_full_encoded_bytes", "property_value_append_encoded_bytes", "property_value_foreground_rollup", "property_value_rollup_encode", "property_value_reconstruct", "property_value_reconstructed_records"});
      endif
    endfor
    this:log_top_counter_deltas(label + ":ops", all_delta, 20);
  endmethod

  method abort owner: ARCH_WIZARD
    ":abort() => NONE";
    "  Emergency stop - cleans up and resets state.";
    this.running = 0;
    this:cleanup();
    player:tell("Benchmark aborted.");
  endmethod

  method configure owner: ARCH_WIZARD
    ":configure(?cycles_per_sample, ?samples_per_level, ?subscriber_counts, ?work_iterations)";
    "  Configure benchmark parameters. All args optional.";
    if (length(args) >= 1)
      this.cycles_per_sample = args[1];
    endif
    if (length(args) >= 2)
      this.samples_per_level = args[2];
    endif
    if (length(args) >= 3)
      this.subscriber_counts = args[3];
    endif
    if (length(args) >= 4)
      this.work_iterations = args[4];
    endif
    player:tell("Config: cycles_per_sample=" + tostr(this.cycles_per_sample) + " samples_per_level=" + tostr(this.samples_per_level) + " counts=" + toliteral(this.subscriber_counts) + " work_iterations=" + tostr(this.work_iterations));
  endmethod

  method test_run_bench owner: ARCH_WIZARD
    "Entry point for running benchmark via test harness.";
    $game_update:start();
    this:run();
    $game_update:stop();
  endmethod

  method test_write_stress owner: ARCH_WIZARD
    "Stress test for write throughput. Creates many subscribers at high Hz.";
    "Optional args: duration, subscribers, work_per_tick, append_mode (defaults: 30, 256, 20, 0)";
    server_log("=== WRITE STRESS TEST STARTING ===");
    "Configure for aggressive write stress testing";
    run_duration = length(args) > 0 ? tofloat(args[1]) | 30.0;
    target_subscribers = length(args) > 1 ? toint(args[2]) | 256;
    work_per_tick = length(args) > 2 ? toint(args[3]) | 20;
    append_mode = length(args) > 3 ? toint(args[4]) | 0;
    update_hz = 50.0;
    "Set up the game update loop with high Hz";
    $game_update.update_hertz = update_hz;
    $game_update.stats_interval = 5.0;
    server_log("Update Hz: " + tostr(update_hz));
    "Create subscribers";
    server_log("Creating " + tostr(target_subscribers) + " subscribers (append_mode=" + tostr(append_mode) + ")...");
    this.subscribers = {};
    for i in [1..target_subscribers]
      sub = create($bench_subscriber, $arch_wizard);
      sub.work_iterations = work_per_tick;
      sub.append_mode = append_mode;
      this.subscribers = {@this.subscribers, sub};
      $game_update:register(sub);
      if (i % 50 == 0)
        server_log("Created " + tostr(i) + " subscribers...");
        commit();
      endif
    endfor
    commit();
    "Start the loop";
    counter_before = this:capture_perf_counters();
    prop_cache_before = this:capture_property_cache_stats();
    verb_cache_before = this:capture_verb_cache_stats();
    $game_update:start();
    writes_per_second = tofloat(target_subscribers * work_per_tick) * update_hz;
    server_log("=== WRITE STRESS TEST RUNNING ===");
    server_log("Subscribers: " + tostr(target_subscribers));
    server_log("Work per tick: " + tostr(work_per_tick));
    server_log("Target Hz: " + tostr(update_hz));
    server_log("Estimated writes/sec: " + tostr(writes_per_second));
    server_log("Running for " + tostr(run_duration) + " seconds...");
    "Let it run";
    suspend(run_duration);
    "Stop and cleanup";
    $game_update:stop();
    counter_after = this:capture_perf_counters();
    prop_cache_after = this:capture_property_cache_stats();
    verb_cache_after = this:capture_verb_cache_stats();
    this:log_perf_delta("write_stress", counter_before, counter_after);
    this:log_property_cache_delta("write_stress", prop_cache_before, prop_cache_after);
    this:log_verb_cache_delta("write_stress", verb_cache_before, verb_cache_after);
    if ($game_update.loop_error || length($game_update.subscriber_faults))
      raise(E_INVARG, "Write stress update loop failed", {$game_update.loop_error, $game_update.subscriber_faults});
    endif
    mutations = 0;
    for sub in (this.subscribers)
      if (sub.counter == 0)
        raise(E_INVARG, "Write stress subscriber did no work", sub);
      endif
      mutations = mutations + sub.counter;
    endfor
    server_log("WRITE_STRESS_RESULT mode=" + tostr(append_mode) + " seconds=" + tostr(run_duration) + " mutations=" + tostr(mutations));
    this:cleanup();
    server_log("=== WRITE STRESS TEST COMPLETE ===");
  endmethod

  method make_history_entry owner: ARCH_WIZARD
    ":make_history_entry(INT entry_bytes) => STR";
    {entry_bytes} = args;
    chunk = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ[]";
    entry = chunk;
    while (length(entry) < entry_bytes)
      entry = entry + entry;
    endwhile
    return entry[1..entry_bytes];
  endmethod

  method make_numbered_history_entry owner: ARCH_WIZARD
    ":make_numbered_history_entry(STR entry, INT entry_bytes, INT writer_number, INT entry_number) => STR";
    {entry, entry_bytes, writer_number, entry_number} = args;
    suffix = ":" + tostr(writer_number) + ":" + tostr(entry_number);
    if (length(suffix) >= entry_bytes)
      return suffix[1..entry_bytes];
    endif
    return entry[1..entry_bytes - length(suffix)] + suffix;
  endmethod

  method test_string_history_append owner: ARCH_WIZARD
    "Append strings to large list properties from independent tasks.";
    "Optional args: writers, initial_entries, entry_bytes, appends_per_writer, append_delay, settle_seconds, append_width, mutation_mode";
    writer_count = length(args) > 0 ? toint(args[1]) | 4;
    initial_entries = length(args) > 1 ? toint(args[2]) | 1024;
    entry_bytes = length(args) > 2 ? toint(args[3]) | 2048;
    appends_per_writer = length(args) > 3 ? toint(args[4]) | 20;
    append_delay = length(args) > 4 ? tofloat(args[5]) | 0.0;
    settle_seconds = length(args) > 5 ? tofloat(args[6]) | 5.0;
    append_width = length(args) > 6 ? toint(args[7]) | 1;
    mutation_mode = length(args) > 7 ? toint(args[8]) | 0;
    if (writer_count < 1 || initial_entries < 1 || entry_bytes < 1 || appends_per_writer < 1 || append_delay < 0.0 || settle_seconds < 0.0 || append_width < 1 || mutation_mode < 0 || mutation_mode > 2)
      raise(E_INVARG, "History benchmark arguments must be positive. Delays can be zero.");
    endif
    server_log("=== STRING HISTORY APPEND BENCHMARK STARTING ===");
    server_log("Writers: " + tostr(writer_count));
    server_log("Initial entries per writer: " + tostr(initial_entries));
    server_log("Entry bytes: " + tostr(entry_bytes));
    server_log("Appends per writer: " + tostr(appends_per_writer));
    server_log("Append delay: " + tostr(append_delay));
    server_log("Append width: " + tostr(append_width));
    server_log("Mutation mode: " + tostr(mutation_mode));
    total_initial_bytes = writer_count * initial_entries * entry_bytes;
    total_appends = writer_count * appends_per_writer;
    server_log("Initial string bytes: " + tostr(total_initial_bytes));
    server_log("Total measured appends: " + tostr(total_appends));

    "Phased moorc runs wait for applied and durable prefixes between seed, producers, and reporting.";
    phase = length(args) > 8 ? toint(args[9]) | 0;
    if (phase <= 1)
      entry = this:make_history_entry(entry_bytes);
      this.subscribers = {};
      for writer_number in [1..writer_count]
        seed_history = {this:make_numbered_history_entry(entry, entry_bytes, writer_number, entry_number) for entry_number in [1..initial_entries]};
        writer = create(#667, #2);
        writer.history_entry = this:make_numbered_history_entry(entry, entry_bytes, writer_number, 0);
        writer.history_append_width = append_width;
        writer.history_mutation_mode = mutation_mode;
        writer.history_running = 1;
        writer.string_history = seed_history;
        this.subscribers = {@this.subscribers, writer};
        commit();
      endfor

      if (phase == 1)
        return 0;
      endif
    endif

    if (phase <= 2)
      "Phased runs drain seeding before these counters.";
      suspend(settle_seconds);
      counter_before = this:capture_perf_counters();
      started_at = ftime(true);
      for writer in (this.subscribers)
        writer:start_string_history_appends(appends_per_writer, append_delay);
      endfor

      deadline = time() + 300;
      running = writer_count;
      while (running > 0 && time() < deadline)
        suspend(0.01);
        running = 0;
        for writer in (this.subscribers)
          if (writer.history_running)
            running = running + 1;
          endif
        endfor
      endwhile
      if (running > 0)
        raise(E_QUOTA, "History benchmark did not complete within 300 seconds.");
      endif
      expected_entries = mutation_mode == 1 ? initial_entries | initial_entries + appends_per_writer * append_width;
      for writer in (this.subscribers)
        if (length(writer.string_history) != expected_entries)
          raise(E_QUOTA, "A history writer did not complete all appends.");
        endif
      endfor

      elapsed = ftime(true) - started_at;
      if (phase == 2)
        return {counter_before, elapsed};
      endif
      suspend(settle_seconds);
    else
      {counter_before, elapsed} = args[10];
    endif
    counter_after = this:capture_perf_counters();
    this:log_perf_delta("string_history_append", counter_before, counter_after);
    server_log("HISTORY_APPEND_RESULT writers=" + tostr(writer_count) + " initial_entries=" + tostr(initial_entries) + " entry_bytes=" + tostr(entry_bytes) + " appends=" + tostr(total_appends) + " append_width=" + tostr(append_width) + " mutation_mode=" + tostr(mutation_mode) + " producer_elapsed_seconds=" + tostr(elapsed));
    for writer in (this.subscribers)
      if (valid(writer))
        recycle(writer);
      endif
    endfor
    this.subscribers = {};
    server_log("=== STRING HISTORY APPEND BENCHMARK COMPLETE ===");
  endmethod

  method test_combat_stress owner: ARCH_WIZARD
    "Stress test that approximates bitmuse-style combat/update load.";
    "Optional args: duration, subscribers, rounds_per_tick, fanout, checks_per_round, state_writes, delay_ratio, update_hz";
    server_log("=== COMBAT STRESS TEST STARTING ===");
    run_duration = length(args) > 0 ? tofloat(args[1]) | 30.0;
    target_subscribers = length(args) > 1 ? toint(args[2]) | 256;
    rounds_per_tick = length(args) > 2 ? toint(args[3]) | 20;
    fanout = length(args) > 3 ? toint(args[4]) | 4;
    checks_per_round = length(args) > 4 ? toint(args[5]) | 2;
    state_writes = length(args) > 5 ? toint(args[6]) | 4;
    delay_ratio = length(args) > 6 ? toint(args[7]) | 10;
    update_hz = length(args) > 7 ? tofloat(args[8]) | 20.0;
    $game_update.update_hertz = update_hz;
    $game_update.stats_interval = 5.0;
    server_log("Update Hz: " + tostr(update_hz));
    server_log("Creating " + tostr(target_subscribers) + " combat subscribers...");
    for i in [1..target_subscribers]
      sub = create($bench_subscriber, $arch_wizard);
      sub.mode = 2;
      sub.work_iterations = rounds_per_tick;
      sub.fanout = fanout;
      sub.fanout_direct_mode = 1;
      sub.checks_per_tick = checks_per_round;
      sub.state_writes = state_writes;
      sub.delay_ratio = delay_ratio;
      $game_update:register(sub);
      this.subscribers = {@this.subscribers, sub};
      if (i % 50 == 0)
        server_log("Created " + tostr(i) + " subscribers...");
        commit();
      endif
    endfor
    commit();
    counter_before = this:capture_perf_counters();
    prop_cache_before = this:capture_property_cache_stats();
    verb_cache_before = this:capture_verb_cache_stats();
    $game_update:start();
    rounds_per_second = tofloat(target_subscribers * rounds_per_tick) * update_hz;
    fanout_calls_per_second = rounds_per_second * tofloat(fanout);
    server_log("=== COMBAT STRESS TEST RUNNING ===");
    server_log("Subscribers: " + tostr(target_subscribers));
    server_log("Rounds per tick: " + tostr(rounds_per_tick));
    server_log("Fanout per round: " + tostr(fanout));
    server_log("Checks per round: " + tostr(checks_per_round));
    server_log("State writes per round: " + tostr(state_writes));
    server_log("Delay ratio (%): " + tostr(delay_ratio));
    server_log("Estimated rounds/sec: " + tostr(rounds_per_second));
    server_log("Estimated fanout calls/sec: " + tostr(fanout_calls_per_second));
    server_log("Running for " + tostr(run_duration) + " seconds...");
    suspend(run_duration);
    $game_update:stop();
    counter_after = this:capture_perf_counters();
    prop_cache_after = this:capture_property_cache_stats();
    verb_cache_after = this:capture_verb_cache_stats();
    this:log_perf_delta("combat_stress", counter_before, counter_after);
    this:log_property_cache_delta("combat_stress", prop_cache_before, prop_cache_after);
    this:log_verb_cache_delta("combat_stress", verb_cache_before, verb_cache_after);
    this:cleanup();
    server_log("=== COMBAT STRESS TEST COMPLETE ===");
  endmethod
endobject
