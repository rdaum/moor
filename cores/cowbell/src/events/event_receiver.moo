object EVENT_RECEIVER [
  import_export_id -> "event_receiver",
  import_export_hierarchy -> {"events"}
]
  name: "Generic Event Receiver"
  parent: ACTOR
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Generic event receiver prototype providing event broadcasting and connection notification capabilities.";

  method tell owner: HACKER
    "Resolve one viewer document, then send its Djot history and negotiated live representations.";
    const {event, @rest} = args;
    this:_is_gagged_event(event) && return;
    const document = this:_resolve_event(event);
    let event_slots = flyslots(event);
    event_slots["delivery_id"] = uuid();
    const history = $document:render(document, 'text_djot);
    let log_slots = event_slots;
    log_slots["annotations"] = history['annotations];
    this:_event_log(this, history['body], 'text_djot, log_slots);
    const connections = this:_connections();
    const contents = this:_event_render(connections, event, document);
    for content in (contents)
      const {connection, content_type, body, annotations} = content;
      let metadata = event_slots;
      metadata["annotations"] = annotations;
      try
        this:_notify(connection, body, false, false, content_type, metadata);
      except error (ANY)
        this:_delivery_error("Event notification failed: " + toliteral(error));
      endtry
    endfor
  endmethod

  method _delivery_error owner: ARCH_WIZARD
    "Log a receiver's delivery failure without granting its content hooks wizard permissions.";
    caller == this || raise(E_PERM);
    const {message} = args;
    server_log(tostr(this) + ": " + message);
  endmethod

  method _resolve_event owner: ARCH_WIZARD
    "Resolve content with this receiver's permissions. Context never inherits delivery's wizard owner.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const {event} = args;
    return event:resolve(this);
  endmethod

  method inform_connection owner: HACKER
    "Deliver utility output only to the requested attached connection. Unavailable destinations are discarded.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {connection_obj, original_event} = args;
    const info = `this:_connection_entry(connection_obj) ! E_INVARG => 0';
    info == 0 && return 0;
    const event = original_event:with_audience('utility);
    const contents = this:_event_render({info}, event);
    const event_slots = flyslots(event);
    for content in (contents)
      const {conn, content_type, output, annotations} = content;
      try
        let rendered_slots = event_slots;
        rendered_slots["annotations"] = annotations;
        this:_notify(conn, output, false, false, content_type, rendered_slots);
      except (E_PERM)
        "Rendering may suspend while the destination disconnects or changes player.";
        return 0;
      endtry
    endfor
    return 0;
  endmethod

  method inform_current owner: HACKER
    "Deliver utility output only to the initiating connection. Never substitute another connection.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {event} = args;
    const current = `connection() ! E_INVARG => 0';
    typeof(current) != TYPE_OBJ && return 0;
    return this:inform_connection(current, event);
  endmethod

  method _can_inform_as owner: HACKER
    "Return whether actor/perms may send utility output for this receiver.";
    {actor, actor_perms} = args;
    if (player == this)
      return true;
    endif
    if (valid(this.location) && actor == this.location)
      return true;
    endif
    if (actor == this || actor == #0)
      return true;
    endif
    if (typeof(actor) == TYPE_OBJ && valid(actor) && actor.wizard)
      return true;
    endif
    if (typeof(actor_perms) == TYPE_OBJ && valid(actor_perms) && actor_perms.wizard)
      return true;
    endif
    return false;
  endmethod

  method _connections owner: ARCH_WIZARD
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    set_task_perms(this);
    return connections(this);
  endmethod

  method _event_render owner: ARCH_WIZARD
    "Serialize one resolved document for attached connections, reusing each negotiated representation.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const {connections, event, ?resolved = false} = args;
    const document = typeof(resolved) == TYPE_FLYWEIGHT ? resolved | event:resolve(this);
    let results = {};
    let rendered = [];
    const types = ["text/html" -> 'text_html, "text/djot" -> 'text_djot,
      "text/plain" -> 'text_plain, "text_html" -> 'text_html,
      "text_djot" -> 'text_djot, "text_plain" -> 'text_plain];
    for connection in (connections)
      const {connection_obj, peer_addr, idle_seconds, content_types, @rest} = connection;
      const supported = length(content_types) ? content_types | {'text_plain};
      let preferred = event:preferred_content_types();
      if (typeof(preferred) != TYPE_LIST || !length(preferred))
        preferred = {'text_html, 'text_djot, 'text_plain};
      endif
      let selected = false;
      for desired in ({@preferred, 'text_html, 'text_djot, 'text_plain})
        const wanted = tostr(desired);
        if (!maphaskey(types, wanted))
          continue;
        endif
        for offered in (supported)
          const available = tostr(offered);
          if (maphaskey(types, available) && types[available] == types[wanted])
            selected = types[wanted];
            break;
          endif
        endfor
        if (typeof(selected) == TYPE_SYM)
          break;
        endif
      endfor
      if (typeof(selected) != TYPE_SYM)
        this:_delivery_error("Connection has no supported narrative content type: " + tostr(connection_obj));
        continue;
      endif
      if (!maphaskey(rendered, selected))
        rendered[selected] = $document:render(document, selected);
      endif
      const result = rendered[selected];
      if (length(result['body]))
        const mime = tosym(strsub(tostr(selected), "_", "/", 1));
        results = {@results, {connection_obj, mime, result['body], result['annotations]}};
      endif
    endfor
    return results;
  endmethod

  method _connection_entry owner: ARCH_WIZARD
    "Locate the connection info tuple for the given connection object or return E_INVARG.";
    set_task_perms(this);
    {connection_obj} = args;
    for info in (connections(this))
      if (info[1] == connection_obj)
        return info;
      endif
    endfor
    raise(E_INVARG, "Connection is not attached to this player.");
  endmethod

  method _notify owner: ARCH_WIZARD
    "Send only to a connection still attached to this receiver after rendering callbacks.";
    caller == this || raise(E_PERM);
    const {target_connection, @rest} = args;
    let attached = false;
    for info in (`connections(this) ! E_INVARG => {}')
      if (info[1] == target_connection)
        attached = true;
        break;
      endif
    endfor
    attached || raise(E_PERM);
    notify(@args);
  endmethod

  method _present owner: ARCH_WIZARD
    "Present receiver-owned UI only to this receiver.";
    caller == this || raise(E_PERM);
    const {target, @rest} = args;
    target == this || raise(E_PERM);
    present(@args);
  endmethod

  method _event_log owner: ARCH_WIZARD
    "Append narrative history only for this receiver.";
    caller == this || raise(E_PERM);
    const {target, @rest} = args;
    target == this || raise(E_PERM);
    event_log(@args);
  endmethod

  method rewrite_event owner: ARCH_WIZARD
    "Replace utility output on its captured connection. Stale destinations never become broadcasts.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {rewrite_id, new_content, ?requested_connection = 0} = args;
    const target_conn = typeof(requested_connection) == TYPE_OBJ ? requested_connection | `connection() ! E_INVARG => 0';
    typeof(target_conn) != TYPE_OBJ && return 0;
    let event;
    if (typeof(new_content) == TYPE_FLYWEIGHT && new_content.delegate == $event)
      event = new_content;
    else
      event = $event:mk_rewrite(this, new_content);
    endif
    event = event:with_metadata('rewrite_target, rewrite_id):with_audience('utility);
    return this:inform_connection(target_conn, event);
  endmethod

  method _is_gagged_event owner: ARCH_WIZARD
    "Return true if this receiver should suppress the event due to gagging.";
    "Primary check: event metadata (dm_from / actor).";
    "Secondary check: callers() chain for initiating player/object (LambdaCore-style).";
    {event} = args;
    "Fast path: if both lists are empty, nothing to do.";
    try
      gl = this.gaglist;
    except e (E_PROPNF)
      gl = {};
    endtry
    try
      ogl = this.object_gaglist;
    except e2 (E_PROPNF)
      ogl = {};
    endtry
    if (!gl && !ogl)
      return false;
    endif
    "Event-based detection.";
    if (typeof(event) == TYPE_FLYWEIGHT)
      slots = flyslots(event);
      source = 0;
      candidate = 0;
      "Prefer explicit source metadata for receiver-personalized events (e.g. DMs).";
      try
        candidate = slots['dm_from];
      except e3 (ANY)
        candidate = 0;
      endtry
      if (typeof(candidate) == TYPE_OBJ && valid(candidate))
        source = candidate;
      else
        candidate = 0;
        try
          candidate = slots['actor];
        except e4 (ANY)
          candidate = 0;
        endtry
        if (typeof(candidate) == TYPE_OBJ && valid(candidate))
          source = candidate;
        endif
      endif
      if (typeof(source) == TYPE_OBJ)
        "Never suppress your own output.";
        if (source == this)
          return false;
        endif
        if (is_player(source))
          if (source in gl)
            return true;
          endif
        else
          if (source in ogl)
            return true;
          endif
        endif
      endif
    endif
    "LambdaCore-style callers()-based detection.";
    for frame in (callers())
      {frame_this, verb_name, programmer, verb_loc, frame_player, line_number} = frame;
      if (typeof(frame_player) == TYPE_OBJ && valid(frame_player) && frame_player != this)
        if (frame_player in gl)
          return true;
        endif
      endif
      if (typeof(frame_this) == TYPE_OBJ && valid(frame_this) && frame_this != this)
        if (frame_this in ogl)
          return true;
        endif
      endif
      if (typeof(verb_loc) == TYPE_OBJ && valid(verb_loc) && verb_loc != this)
        if (verb_loc in ogl)
          return true;
        endif
      endif
    endfor
    return false;
  endmethod

  method emit_room_look owner: ARCH_WIZARD
    "Emit room look to narrative notify plus OOB room panel state.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    {?room = this.location} = args;
    if (!valid(room) || !isa(room, $room))
      room = this.location;
    endif
    if (!valid(room) || !isa(room, $room))
      return;
    endif
    look_d = room:look_self();
    look_event = look_d:into_event();
    look_event = look_event:with_metadata('look_kind, "room");
    look_event = look_event:with_metadata('look_room, room);
    "Emit OOB structured room state for this viewer only.";
    `this:emit_room_snapshot_state(this, room, look_d) ! ANY';
    "Always emit the existing narrative event path.";
    this:inform_current(look_event);
  endmethod

  method emit_room_snapshot_state owner: ARCH_WIZARD
    "Emit room snapshot state to a specific viewer.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    {viewer, room, ?look_d = 0} = args;
    if (!valid(viewer) || !is_player(viewer))
      return;
    endif
    if (!valid(room) || !isa(room, $room))
      return;
    endif
    try
      snapshot = room:room_snapshot(viewer, look_d);
      emit_data(viewer, 'state, 'room_snapshot, snapshot);
    except e (ANY)
      "Best-effort snapshot broadcast.";
    endtry
  endmethod

  method test_can_inform_as_checks_explicit_authority owner: HACKER
    "Inform permission checks should use the supplied actor/perms, not the helper's caller.";
    receiver = $test_utils:anonymous($event_receiver);
    room = $test_utils:anonymous($room);
    unrelated = $test_utils:anonymous($thing);
    try
      move(receiver, room);
      $test_utils:assert_true(receiver:_can_inform_as(receiver, $hacker), "receiver should inform itself");
      $test_utils:assert_true(receiver:_can_inform_as(room, $hacker), "receiver location should inform the receiver");
      $test_utils:assert_true(receiver:_can_inform_as(unrelated, $arch_wizard), "wizard perms should inform a receiver");
      $test_utils:assert_false(receiver:_can_inform_as(unrelated, $hacker), "unrelated non-wizard actor should not inform a receiver");
    finally
      valid(receiver) && move(receiver, $prototype_box);
      $test_utils:destroy_if_valid(room);
    endtry
    return true;
  endmethod

  method test_inform_methods_reject_unrelated_receiver owner: HACKER
    "Public inform helpers should reject calls aimed at an unrelated receiver.";
    receiver = $test_utils:anonymous($event_receiver);
    event = $event:mk_info(receiver, "hidden");
    $test_utils:assert_raises(E_PERM, receiver, "inform_current", {event}, "inform_current should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "_connections", {}, "_connections should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "rewrite_event", {"rewrite-id", "replacement"}, "rewrite_event should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "emit_room_snapshot_state", {receiver, $prototype_box}, "emit_room_snapshot_state should reject unrelated callers");
    return true;
  endmethod
endobject
