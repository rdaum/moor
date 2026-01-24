object BENCH_CONTROLLER
  name: "Game Update Benchmark Controller"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property subscribers (owner: ARCH_WIZARD, flags: "rw") = {};
  property running (owner: ARCH_WIZARD, flags: "r") = 0;
  property cycles_per_sample (owner: ARCH_WIZARD, flags: "rw") = 20;
  property samples_per_level (owner: ARCH_WIZARD, flags: "rw") = 3;
  property subscriber_counts (owner: ARCH_WIZARD, flags: "rw") = {1, 4, 16, 64, 256, 512, 1024};
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;

  override import_export_hierarchy = {};
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
      for i in [1..(target - current)]
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
endobject
