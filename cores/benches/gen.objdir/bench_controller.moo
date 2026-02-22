object BENCH_CONTROLLER
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

  override import_export_id = "bench_controller";

  verb run (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb set_subscriber_count (this none this) owner: ARCH_WIZARD flags: "rxd"
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
        this.subscribers = (this.subscribers)[1..$ - 1];
      endfor
    endif
    commit();
    "Let things settle";
    suspend(0.5);
  endverb

  verb run_samples (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb cleanup (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb capture_perf_counters (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":capture_perf_counters() => MAP";
    snapshot = [];
    snapshot["bf"] = bf_counters();
    snapshot["db"] = db_counters();
    snapshot["sched"] = sched_counters();
    return snapshot;
  endverb

  verb counter_delta (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb log_top_counter_deltas (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb log_perf_delta (this none this) owner: ARCH_WIZARD flags: "rxd"
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
    endfor
    this:log_top_counter_deltas(label + ":ops", all_delta, 20);
  endverb

  verb abort (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":abort() => NONE";
    "  Emergency stop - cleans up and resets state.";
    this.running = 0;
    this:cleanup();
    player:tell("Benchmark aborted.");
  endverb

  verb configure (this none this) owner: ARCH_WIZARD flags: "rxd"
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
  endverb

  verb test_run_bench (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Entry point for running benchmark via test harness.";
    $game_update:start();
    this:run();
    $game_update:stop();
  endverb

  verb test_write_stress (this none this) owner: ARCH_WIZARD flags: "rxd"
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
    for i in [1..target_subscribers]
      sub = create($bench_subscriber, $arch_wizard);
      sub.work_iterations = work_per_tick;
      sub.append_mode = append_mode;
      $game_update:register(sub);
      if (i % 50 == 0)
        server_log("Created " + tostr(i) + " subscribers...");
        commit();
      endif
    endfor
    commit();
    "Start the loop";
    counter_before = this:capture_perf_counters();
    $game_update:start();
    writes_per_second = target_subscribers * work_per_tick * update_hz;
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
    this:log_perf_delta("write_stress", counter_before, counter_after);
    this:cleanup();
    server_log("=== WRITE STRESS TEST COMPLETE ===");
  endverb

  verb test_combat_stress (this none this) owner: ARCH_WIZARD flags: "rxd"
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
    $game_update:start();
    rounds_per_second = target_subscribers * rounds_per_tick * update_hz;
    fanout_calls_per_second = rounds_per_second * fanout;
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
    this:log_perf_delta("combat_stress", counter_before, counter_after);
    this:cleanup();
    server_log("=== COMBAT STRESS TEST COMPLETE ===");
  endverb
endobject