object #666
  name: "Game Update System"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property last_stats_time (owner: ARCH_WIZARD, flags: "r") = 0.0;
  property latency (owner: ARCH_WIZARD, flags: "r") = {};
  property running (owner: ARCH_WIZARD, flags: "r") = 0;
  property stats_interval (owner: ARCH_WIZARD, flags: "r") = 10.0;
  property subscriber_faults (owner: ARCH_WIZARD, flags: "r") = [];
  property subscribers (owner: ARCH_WIZARD, flags: "r") = {};
  property task_id (owner: ARCH_WIZARD, flags: "r") = 0;
  property tick_count (owner: ARCH_WIZARD, flags: "r") = 0;
  property update_commit_interval (owner: ARCH_WIZARD, flags: "r") = 20;
  property update_hertz (owner: ARCH_WIZARD, flags: "r") = 20.0;

  verb process_updates (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Based on bitMuse game update loop, adapter for benchmarking game-tick type update loops.";
    ":process_updates() => NONE";
    "  The meat of the loop that runs :fixed_update(@state)";
    if (task_id() != this.task_id)
      "guard statement that allows us to abort if someone called this on the wrong task";
      "it's okay if this happens here we can silently abort";
      return;
    endif
    update_delays = [];
    update_state = [];
    interrupts = {};
    this.tick_count = 0;
    this.last_stats_time = ftime();
    while (this.running)
      try
        if (task_id() != this.task_id)
          server_log(toliteral(this) + " (" + tostr(task_id()) + ") somehow ended up processing updates on the wrong task (expected " + tostr(this.task_id) + "). Aborting.");
          return;
        endif
        start_time = ftime();
        for subscriber, i in (this.subscribers)
          if (!valid(subscriber))
            this.subscribers = setremove(this.subscribers, subscriber);
            update_delays = `mapdelete(update_delays, subscriber) ! ANY => update_delays';
            update_state = `mapdelete(update_state, subscriber) ! ANY => update_state';
            interrupts = setremove(interrupts, subscriber);
            continue;
          endif
          if (`update_delays[subscriber] ! E_RANGE => 0.0' > start_time)
            "check for interrupts";
            if (!(subscriber in interrupts))
              "sleeping subscribers remain asleep";
              continue;
            endif
            interrupts = setremove(interrupts, subscriber);
          endif
          try
            player = subscriber;
            result = subscriber:fixed_update(@`update_state[subscriber] ! ANY => {}');
            !(i == 0 || i % this.update_commit_interval) && commit();
            if (result)
              {delay_ms, new_state} = result;
              update_state[subscriber] = new_state;
              update_delays[subscriber] = ftime() + delay_ms;
            else
              "clearing delay cache here removes stale values";
              update_delays = `mapdelete(update_delays, subscriber) ! ANY => update_delays';
              update_state = `mapdelete(update_state, subscriber) ! ANY => update_state';
            endif
          except e (ANY)
            if (maphaskey(this.subscriber_faults, subscriber))
              continue;
            endif
            server_log(toliteral(e));
            this.subscriber_faults[subscriber] = e;
          endtry
        endfor
        runtime = ftime() - start_time;
        update_time = 1.0 / this.update_hertz;
        this.latency = {runtime, @this.latency}[1..min(500, $)];
        this.tick_count = this.tick_count + 1;
        "Periodic stats logging";
        now = ftime();
        if (now - this.last_stats_time >= this.stats_interval)
          elapsed = now - this.last_stats_time;
          ticks = this.tick_count;
          actual_hz = ticks / elapsed;
          sub_count = length(this.subscribers);
          mean_lat = 0.0;
          if (length(this.latency) > 0)
            for lat in (this.latency)
              mean_lat = mean_lat + lat;
            endfor
            mean_lat = mean_lat / length(this.latency);
          endif
          server_log("TICK_STATS ticks=" + tostr(ticks) + " elapsed=" + tostr(elapsed) + " actual_hz=" + tostr(actual_hz) + " target_hz=" + tostr(this.update_hertz) + " subscribers=" + tostr(sub_count) + " mean_latency_ms=" + tostr(mean_lat * 1000.0));
          this.tick_count = 0;
          this.last_stats_time = now;
        endif
        messages = task_recv(max(0.0, update_time - runtime));
        for message in (messages)
          {operation, obj_id} = message;
          if (operation == 1)
            "interrupt";
            interrupts = setadd(interrupts, obj_id);
          elseif (operation == 2)
            "subscriber add";
            this.subscribers = setadd(this.subscribers, obj_id);
          elseif (operation == 3)
            "subscriber remove";
            this.subscribers = setremove(this.subscribers, obj_id);
            update_delays = `mapdelete(update_delays, obj_id) ! ANY => update_delays';
            update_state = `mapdelete(update_state, obj_id) ! ANY => update_state';
            interrupts = setremove(interrupts, obj_id);
          endif
        endfor
      except e (ANY)
        server_log(toliteral(e));
        suspend(5);
      endtry
    endwhile
  endverb

  verb subscribers (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":subscribers() => LIST of OBJs";
    "  Returns all valid subscribers";
    for subscriber in (this.subscribers)
      if (!valid(subscriber))
        this.subscribers = setremove(this.subscribers, subscriber);
      endif
    endfor
    commit();
    return this.subscribers;
  endverb

  verb register (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":register(OBJ subscriber) => NONE";
    "  Adds a subscriber to the subscriber pool for regular fixed updates";
    {subscriber} = args;
    if (!valid(subscriber))
      return;
    elseif (!respond_to(subscriber, "fixed_update"))
      return;
    endif
    "Route registration through mailbox to mirror bitmuse loop ownership.";
    if (!(this.running && valid_task(this.task_id)))
      this:start();
    endif
    task_send(this.task_id, {2, subscriber});
  endverb

  verb start (this none this) owner: ARCH_WIZARD flags: "rxd"
    if (valid_task(this.task_id) && this.running)
      "we are already running";
      return;
    endif
    try
      server_log("We are starting a new task because valid_task says task_id " + toliteral(this.task_id) + " is not valid; but it is.");
      server_log("Active Tasks: " + toliteral(active_tasks()));
      server_log("Queued Tasks: " + toliteral(queued_tasks()));
    except e (ANY)
      server_log(toliteral(e));
    endtry
    this.running = true;
    fork process (1)
      commit();
      this:process_updates();
    endfork
    this.task_id = process;
    commit();
  endverb

  verb stop (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Stop the game update loop.";
    caller_perms().wizard || raise(E_PERM);
    if (!this.running || !valid_task(this.task_id))
      this.running = 0;
      return false;
    endif
    if (this.task_id)
      `kill_task(this.task_id) ! ANY';
      this.task_id = 0;
    endif
    this.running = 0;
    return true;
  endverb

  verb unregister (this none this) owner: ARCH_WIZARD flags: "rxd"
    {subscriber} = args;
    if (this.running && valid_task(this.task_id))
      task_send(this.task_id, {3, subscriber});
    else
      this.subscribers = setremove(this.subscribers, subscriber);
    endif
  endverb

  verb interrupt (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":interrupt(OBJ subscriber) => NONE";
    "  Interrupts tell the fixed update to clear any suspends on a task";
    "  This is useful for cases where we might be waiting and want to immediately execute";
    {subscriber} = args;
    if (this.running && valid_task(this.task_id))
      task_send(this.task_id, {1, subscriber});
    endif
  endverb

  verb clear_faults (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":clear_faults(?OBJ subscriber) => NONE";
    "  Clears subscriber faults. If subscriber provided, clears only that one.";
    if (length(args) > 0)
      {subscriber} = args;
      this.subscriber_faults = `mapdelete(this.subscriber_faults, subscriber) ! ANY => this.subscriber_faults';
    else
      this.subscriber_faults = [];
    endif
  endverb

  verb resume_if_needed (this none this) owner: ARCH_WIZARD flags: "rxd"
    "Resume game update loop if there are subscribers. Called on server startup.";
    caller == #0 || caller_perms().wizard || raise(E_PERM);
    if (this.running && valid_task(this.task_id))
      return false;
    endif
    if (length(this.subscribers) > 0)
      this:start();
      return true;
    endif
    return false;
  endverb
endobject