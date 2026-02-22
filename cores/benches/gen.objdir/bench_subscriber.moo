object BENCH_SUBSCRIBER
  name: "Benchmark Subscriber"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property append_mode (owner: ARCH_WIZARD, flags: "rw") = 0;
  property checks_per_tick (owner: ARCH_WIZARD, flags: "rw") = 1;
  property cooldowns (owner: ARCH_WIZARD, flags: "rw") = [];
  property counter (owner: ARCH_WIZARD, flags: "rw") = 0;
  property data (owner: ARCH_WIZARD, flags: "rw") = {};
  property defender_momentum (owner: ARCH_WIZARD, flags: "rw") = 50.0;
  property delay_ratio (owner: ARCH_WIZARD, flags: "rw") = 0;
  property fanout (owner: ARCH_WIZARD, flags: "rw") = 0;
  property fanout_cursor (owner: ARCH_WIZARD, flags: "rw") = 1;
  property last_attacker (owner: ARCH_WIZARD, flags: "rw") = #-1;
  property mode (owner: ARCH_WIZARD, flags: "rw") = 0;
  property momentum (owner: ARCH_WIZARD, flags: "rw") = 50.0;
  property state_writes (owner: ARCH_WIZARD, flags: "rw") = 2;
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;

  override import_export_id = "bench_subscriber";

  verb fixed_update (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":fixed_update(@state) => 0";
    "  Performs property mutations to stress transaction handling.";
    "  Runs every tick (no delay).";
    iterations = this.work_iterations;
    if (this.mode == 2)
      this:fixed_update_combat(iterations);
      if (this.delay_ratio > 0 && random(100) <= this.delay_ratio)
        return {1.0 / $game_update.update_hertz, {}};
      endif
    elseif (this.append_mode)
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

  verb fixed_update_combat (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":fixed_update_combat(INT iterations) => NONE";
    {iterations} = args;
    momentum = this.momentum;
    defender_momentum = this.defender_momentum;
    checks_per_tick = max(1, this.checks_per_tick);
    state_writes = max(1, this.state_writes);
    for i in [1..iterations]
      "Approximate contested attack quality.";
      attack_bonus = 10.0 + (momentum - defender_momentum) / 10.0;
      check = attack_bonus - tofloat(random(100));
      if (check >= 0.0)
        damage = 5.0 + check / 20.0;
        this.counter = this.counter + toint(max(0.0, damage));
        momentum = min(100.0, momentum + 3.0);
        defender_momentum = max(0.0, defender_momentum - 2.0);
        this.cooldowns["last_hit"] = this.counter;
      else
        momentum = max(0.0, momentum - 1.0);
        defender_momentum = min(100.0, defender_momentum + 1.0);
        this.cooldowns["last_miss"] = this.counter;
      endif
      "Extra branchy state churn.";
      for j in [1..checks_per_tick]
        scratch = (this.counter + j) % 97;
        if (scratch % 2)
          this.cooldowns[tostr("chk_", j)] = scratch;
        else
          this.cooldowns = `mapdelete(this.cooldowns, tostr("chk_", j)) ! ANY => this.cooldowns';
        endif
      endfor
      "Write a handful of properties/maps each round.";
      for w in [1..state_writes]
        key = tostr("stat_", (w - 1) % 4 + 1);
        this.cooldowns[key] = this.counter + w;
      endfor
      this:fanout_attack(check);
    endfor
    this.momentum = momentum;
    this.defender_momentum = defender_momentum;
  endverb

  verb fanout_attack (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":fanout_attack(FLOAT attack_check) => NONE";
    {attack_check} = args;
    if (this.fanout <= 0)
      return;
    endif
    peers = $bench_controller.subscribers;
    peer_count = length(peers);
    if (peer_count <= 1)
      return;
    endif
    sent = 0;
    cursor = this.fanout_cursor;
    while (sent < this.fanout)
      if (cursor > peer_count)
        cursor = 1;
      endif
      peer = peers[cursor];
      cursor = cursor + 1;
      if (!valid(peer) || peer == this)
        continue;
      endif
      `peer:on_attack(this, attack_check, this.counter) ! E_VERBNF => 0';
      sent = sent + 1;
    endwhile
    this.fanout_cursor = cursor;
  endverb

  verb on_attack (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":on_attack(OBJ attacker, FLOAT attack_check, INT round_counter) => INT";
    {attacker, attack_check, round_counter} = args;
    this.last_attacker = attacker;
    if (attack_check >= 0.0)
      this.counter = this.counter + 1;
    else
      this.counter = max(0, this.counter - 1);
    endif
    if (this.append_mode && length(this.data) < 256)
      this.data = {@this.data, round_counter};
    endif
    return 1;
  endverb

  verb reset (this none this) owner: ARCH_WIZARD flags: "rxd"
    this.counter = 0;
    this.data = {};
    this.cooldowns = [];
    this.momentum = 50.0;
    this.defender_momentum = 50.0;
    this.fanout_cursor = 1;
    this.last_attacker = #-1;
  endverb
endobject