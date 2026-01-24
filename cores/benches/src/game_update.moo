object GAME_UPDATE
  name: "Game Update System"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property latency (owner: ARCH_WIZARD, flags: "r") = {};
  property running (owner: ARCH_WIZARD, flags: "r") = 0;
  property subscriber_faults (owner: ARCH_WIZARD, flags: "r") = [];
  property subscribers (owner: ARCH_WIZARD, flags: "r") = {};
  property task_id (owner: ARCH_WIZARD, flags: "r") = 0;
  property update_hertz (owner: ARCH_WIZARD, flags: "r") = 20.0;
  property interrupts (owner: ARCH_WIZARD, flags: "r") = {};

  override import_export_hierarchy = {};

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
    while (this.running)
      try
        if (task_id() != this.task_id)
          server_log(toliteral(this) + " (" + tostr(task_id()) + ") somehow ended up processing updates on the wrong task (expected " + tostr(this.task_id) + "). Aborting.");
          return;
        endif
        start_time = ftime(true);
        for subscriber in (this:subscribers())
          if (`update_delays[subscriber] ! E_RANGE => 0.0' > start_time)
            "check for interrupts";
            if (!(subscriber in interrupts))
              "sleeping subscribers remain asleep";
              continue;
            endif
            commit();
            this.interrupts = interrupts = setremove(this.interrupts, subscriber);
            commit();
          endif
          try
            player = subscriber;
            result = subscriber:fixed_update(@`update_state[subscriber] ! ANY => {}');
            if (result)
              {delay_ms, new_state} = result;
              update_state[subscriber] = new_state;
              update_delays[subscriber] = ftime(true) + delay_ms;
            else
              "clearing delay cache here removes stale values";
              update_delays = `mapdelete(update_delays, subscriber) ! ANY => update_delays';
            endif
          except e (ANY)
            if (maphaskey(this.subscriber_faults, subscriber))
              continue;
            endif
            server_log(toliteral(e));
            this.subscriber_faults[subscriber] = e;
            commit();
          endtry
        endfor
        runtime = ftime(true) - start_time;
        update_time = 1.0 / this.update_hertz;
        this.latency = {runtime, @this.latency}[1..min(5, $)];
        suspend(max(0.0, update_time - runtime));
        this:prune_interrupts();
        interrupts = this.interrupts;
      except e (ANY)
        server_log(toliteral(e));
        suspend(5);
        this:prune_interrupts();
        interrupts = this.interrupts;
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
    this.subscribers = setadd(this.subscribers, subscriber);
    "This will start the game update loop if it is not already running";
    this:start();
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
    this.subscribers = setremove(this.subscribers, subscriber);
  endverb

  verb interrupt (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":interrupt(OBJ subscriber) => NONE";
    "  Interrupts tell the fixed update to clear any suspends on a task";
    "  This is useful for cases where we might be waiting and want to immediately execute";
    {subscriber} = args;
    this.interrupts = setadd(this.interrupts, subscriber);
  endverb

  verb prune_interrupts (this none this) owner: ARCH_WIZARD flags: "rxd"
    ":prune_interrupts() => NONE";
    "  A function that will go through our interrupts and remove any that are invalid";
    valid_interrupts = {};
    for i in (this.interrupts)
      if (!valid(i))
        continue;
      endif
      valid_interrupts = setadd(valid_interrupts, i);
    endfor
    this.interrupts = valid_interrupts;
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
