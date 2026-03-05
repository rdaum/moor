object BENCH_SUBSCRIBER
  name: "Benchmark Subscriber"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property counter (owner: ARCH_WIZARD, flags: "rw") = 0;
  property data (owner: ARCH_WIZARD, flags: "rw") = {};
  property work_iterations (owner: ARCH_WIZARD, flags: "rw") = 10;
  property append_mode (owner: ARCH_WIZARD, flags: "rw") = 0;
  property mode (owner: ARCH_WIZARD, flags: "rw") = 0;
  property fanout (owner: ARCH_WIZARD, flags: "rw") = 0;
  property fanout_direct_mode (owner: ARCH_WIZARD, flags: "rw") = 0;
  property checks_per_tick (owner: ARCH_WIZARD, flags: "rw") = 1;
  property state_writes (owner: ARCH_WIZARD, flags: "rw") = 2;
  property delay_ratio (owner: ARCH_WIZARD, flags: "rw") = 0;
  property momentum (owner: ARCH_WIZARD, flags: "rw") = 50.0;
  property defender_momentum (owner: ARCH_WIZARD, flags: "rw") = 50.0;
  property cooldowns (owner: ARCH_WIZARD, flags: "rw") = [];
  property fanout_cursor (owner: ARCH_WIZARD, flags: "rw") = 1;
  property last_attacker (owner: ARCH_WIZARD, flags: "rw") = #-1;

  override import_export_hierarchy = {};
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
    counter = this.counter;
    cooldowns = this.cooldowns;
    fanout = this.fanout;
    fanout_direct_mode = this.fanout_direct_mode;
    peers = fanout > 0 ? $bench_controller.subscribers | {};
    peer_count = fanout > 0 ? length(peers) | 0;
    cursor = this.fanout_cursor;
    for i in [1..iterations]
      "Approximate contested attack quality.";
      attack_bonus = 10.0 + (momentum - defender_momentum) / 10.0;
      check = attack_bonus - tofloat(random(100));
      if (check >= 0.0)
        damage = 5.0 + check / 20.0;
        counter = counter + toint(max(0.0, damage));
        momentum = min(100.0, momentum + 3.0);
        defender_momentum = max(0.0, defender_momentum - 2.0);
        cooldowns["last_hit"] = counter;
      else
        momentum = max(0.0, momentum - 1.0);
        defender_momentum = min(100.0, defender_momentum + 1.0);
        cooldowns["last_miss"] = counter;
      endif
      "Extra branchy state churn.";
      for j in [1..checks_per_tick]
        scratch = (counter + j) % 97;
        if (scratch % 2)
          cooldowns[tostr("chk_", j)] = scratch;
        else
          cooldowns = `mapdelete(cooldowns, tostr("chk_", j)) ! ANY => cooldowns';
        endif
      endfor
      "Write a handful of properties/maps each round.";
      for w in [1..state_writes]
        key = tostr("stat_", ((w - 1) % 4) + 1);
        cooldowns[key] = counter + w;
      endfor
      if (fanout > 0 && peer_count > 1)
        sent = 0;
        while (sent < fanout)
          if (cursor > peer_count)
            cursor = 1;
          endif
          peer = peers[cursor];
          cursor = cursor + 1;
          if (!valid(peer) || peer == this)
            continue;
          endif
          if (fanout_direct_mode)
            peer.last_attacker = this;
            if (check >= 0.0)
              peer.counter = peer.counter + 1;
            else
              peer.counter = max(0, peer.counter - 1);
            endif
            if (peer.append_mode && length(peer.data) < 256)
              peer.data = {@peer.data, counter};
            endif
          else
            `peer:on_attack(this, check, counter) ! E_VERBNF => 0';
          endif
          sent = sent + 1;
        endwhile
      endif
    endfor
    this.counter = counter;
    this.cooldowns = cooldowns;
    this.momentum = momentum;
    this.defender_momentum = defender_momentum;
    this.fanout_cursor = cursor;
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
