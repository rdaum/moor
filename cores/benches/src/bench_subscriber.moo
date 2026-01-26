object BENCH_SUBSCRIBER
  name: "Benchmark Subscriber"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property counter (owner: ARCH_WIZARD, flags: "rw") = 0;
  property data (owner: ARCH_WIZARD, flags: "rw") = {};
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;
  property append_mode (owner: ARCH_WIZARD, flags: "rw") = 0;

  override import_export_hierarchy = {};
  override import_export_id = "bench_subscriber";

  verb fixed_update (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":fixed_update(@state) => 0";
    "  Performs property mutations to stress transaction handling.";
    "  Runs every tick (no delay).";
    iterations = this.work_iterations;
    if (this.append_mode)
      "Append mode: grows data to trigger compaction, truncates at 1000 to avoid OOM";
      for i in [1..iterations]
        this.data = {@this.data, this.counter};
        this.counter = this.counter + 1;
      endfor
      if (length(this.data) > 1000)
        this.data = {};
      endif
    else
      "Counter mode: overwrites same value (no growth)";
      for i in [1..iterations]
        this.counter = this.counter + 1;
      endfor
    endif
    return 0;
  endverb

  verb reset (this none this) owner: ARCH_WIZARD flags: "rxd"
    this.counter = 0;
    this.data = {};
  endverb
endobject
