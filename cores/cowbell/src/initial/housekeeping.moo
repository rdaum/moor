object HOUSEKEEPING [
  import_export_id -> "housekeeping",
  import_export_hierarchy -> {"initial"}
]
  name: "Housekeeping"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  property idle_threshold (owner: ARCH_WIZARD, flags: "rc") = 3600;
  property schedule_id (owner: ARCH_WIZARD, flags: "rc") = 0;
  property sweep_interval (owner: ARCH_WIZARD, flags: "rc") = 3000;
  property sweep_msgs (owner: ARCH_WIZARD, flags: "rc") = HOUSEKEEPING_SWEEP_MSGS;

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"sweep_msgs"})
      let bindings = $sub_utils:standard_bindings("The housekeeper.");
      return bindings;
    endif
    return pass(@args);
  endmethod

  method sweep owner: ARCH_WIZARD
    "Sweep disconnected players back to their home rooms.";
    "Native recurring firings use their creator as caller; each firing has a separate transaction.";
    caller_perms().wizard || raise(E_PERM);
    const connected = connected_players();
    const threshold = this.idle_threshold;
    const now = time();
    let swept = {};
    for p in (players())
      if (p in connected)
        continue;
      endif
      const disconnected_at = `p.last_disconnected ! E_PROPNF => 0';
      if (disconnected_at == 0 || now - disconnected_at < threshold)
        continue;
      endif
      let home = `p.home ! E_PROPNF => #-1';
      if (typeof(home) != TYPE_OBJ || !valid(home))
        this:_server_log("Player with no home: " + tostr(p) + " (" + p.name + ") .. resetting to $login.default_home");
        p.home = $login.default_home;
        home = p.home;
      endif
      if (p.location == home)
        continue;
      endif
      const old_loc = p.location;
      try
        p:moveto(home);
        swept = {@swept, {old_loc, p}};
        const msg = "Housekeeping quietly escorted you back to " + home.name + " while you were away.";
        p:tell($event:mk_info(this, msg):with_audience('utility));
      except error (ANY)
        this:_server_log("Housekeeping player error: " + toliteral(error));
      endtry
    endfor
    let rooms_done = {};
    const total_swept = length(swept);
    for entry in (swept)
      const {room, _} = entry;
      if (room in rooms_done)
        continue;
      endif
      rooms_done = {@rooms_done, room};
      let names = {};
      for e in (swept)
        if (e[1] == room)
          names = {@names, e[2].name};
        endif
      endfor
      if (!valid(room) || !respond_to(room, 'announce) || length(names) == 0)
        continue;
      endif
      let msg = "";
      if (length(names) == 1)
        msg = "Housekeeping quietly escorts " + names[1] + " away to bed.";
      elseif (length(names) <= 4)
        msg = "Housekeeping quietly escorts " + names:english_list() + " away to bed.";
      else
        const shown = names[1..3];
        const rest = length(names) - 3;
        msg = "Housekeeping quietly escorts " + shown:english_list() + ", and " + tostr(rest) + " others away to bed.";
      endif
      try
        room:announce($event:mk_info(this, msg):with_audience('utility));
      except error (ANY)
        this:_server_log("Housekeeping room output error: " + toliteral(error));
      endtry
    endfor
    total_swept > 0 && this:_server_log(tostr("Housekeeping swept ", total_swept, " sleeping player(s) home."));
    return total_swept;
  endmethod

  method _server_log owner: ARCH_WIZARD
    "Write a server log entry without keeping the caller's full wizard authority.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this, {{"builtin_call", "server_log"}});
    server_log(@args);
  endmethod

  method start owner: ARCH_WIZARD
    "Start one native housekeeping schedule. Wizard callers only; creation commits with this property.";
    caller_perms().wizard || raise(E_PERM);
    if (schedule_valid(this.schedule_id))
      return "Already running (schedule_id: " + tostr(this.schedule_id) + ")";
    endif
    this.schedule_id = schedule_every(this, "sweep", this.sweep_interval, {}, ['adaptive -> false, 'pass_elapsed -> false]);
    return "Housekeeping started (schedule_id: " + tostr(this.schedule_id) + ", interval: " + tostr(this.sweep_interval) + "s)";
  endmethod

  method stop owner: ARCH_WIZARD
    "Stop future native sweeps. Running firings finish; cancellation commits with the cleared property.";
    caller_perms().wizard || raise(E_PERM);
    const schedule_id = this.schedule_id;
    const stopped = schedule_stop(schedule_id);
    this.schedule_id = 0;
    return stopped ? "Housekeeping stopped (was schedule_id: " + tostr(schedule_id) + ")" | "Not running.";
  endmethod
endobject
