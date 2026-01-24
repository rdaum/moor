object BENCH_SUBSCRIBER
  name: "Benchmark Subscriber"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property counter (owner: ARCH_WIZARD, flags: "rw") = 0;
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;

  override import_export_hierarchy = {};
  override import_export_id = "bench_subscriber";

  verb fixed_update (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":fixed_update(@state) => 0";
    "  Performs property mutations to stress transaction handling.";
    "  Runs every tick (no delay).";
    iterations = this.work_iterations;
    for i in [1..iterations]
      this.counter = this.counter + 1;
    endfor
    return 0;
  endverb

  verb reset (this none this) owner: ARCH_WIZARD flags: "rxd"
    this.counter = 0;
  endverb
endobject
