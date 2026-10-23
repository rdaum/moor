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
    "Deliver and store one captured document, reusing history and connection renderings.";
    const {event, @rest} = args;
    this:_is_gagged_event(event) && return;
    return this:_deliver(event, this:_event_render(this:_connections(), event, true));
  endmethod

  method _deliver owner: HACKER
    "Pair bodies with the shared document and their own annotations, preserving history policy.";
    caller == this || raise(E_PERM);
    const {event, batch} = args;
    let metadata = mapdelete(flyslots(event), 'bindings);
    metadata['delivery_id] = uuid();
    metadata['cowbell_document] = batch['document];
    const history = batch['history];
    if (typeof(history) == TYPE_MAP)
      let log_metadata = metadata;
      log_metadata['annotations] = history['annotations];
      this:_event_log(this, history['body], 'text_djot, log_metadata);
    endif
    for content in (batch['connections])
      const {connection, content_type, body, annotations} = content;
      let rendered_metadata = metadata;
      rendered_metadata['annotations] = annotations;
      try
        this:_notify(connection, body, false, false, content_type, rendered_metadata);
      except (E_PERM)
        "A connection may detach or switch players while content callbacks run.";
      except error (ANY)
        this:_delivery_error("Event notification failed: " + toliteral(error));
      endtry
    endfor
    return 0;
  endmethod

  method _delivery_error owner: ARCH_WIZARD
    "Log a receiver's delivery failure without granting its content hooks wizard permissions.";
    caller == this || raise(E_PERM);
    const {message} = args;
    server_log(tostr(this) + ": " + message);
  endmethod

  method inform_connection owner: HACKER
    "Deliver utility output only to the requested attached connection; detached destinations are discarded.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {connection_obj, original_event} = args;
    const info = `this:_connection_entry(connection_obj) ! E_INVARG => 0';
    info == 0 && return 0;
    const event = original_event:with_audience('utility);
    return this:_deliver(event, this:_event_render({info}, event));
  endmethod

  method inform_all owner: HACKER
    "Deliver utility output to all attached connections without adding player history.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {original_event} = args;
    const event = original_event:with_audience('utility);
    const connections = this:_connections();
    !length(connections) && return 0;
    return this:_deliver(event, this:_event_render(connections, event));
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
    "Authorize utility output for the receiver's principal or its trusted event sources.";
    const {actor, actor_perms} = args;
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
    if (typeof(actor_perms) == TYPE_OBJ && valid(actor_perms) && (actor_perms == this || actor_perms.wizard))
      return true;
    endif
    return false;
  endmethod

  method _connections owner: ARCH_WIZARD
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    set_task_perms(this);
    return `connections(this) ! E_INVARG => {}';
  endmethod

  method _event_render owner: ARCH_WIZARD
    "Resolve and encode once for this receiver, sharing each format between history and connections.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const {connections, event, ?history = false} = args;
    let requests = history ? {['format -> 'text_djot]} | {};
    let destinations = {};
    for entry in (connections)
      const {connection, peer, idle, offered, @rest} = entry;
      let format;
      try
        format = $format:connection_format(offered);
      except error (E_INVARG)
        this:_delivery_error(tostr(connection) + ": " + error[2]);
        continue;
      endtry
      requests = {@requests, ['format -> format]};
      destinations = {@destinations, {connection, length(requests)}};
    endfor
    const batch = event:render_for(this, requests);
    for diagnostic in (batch['diagnostics])
      this:_delivery_error(diagnostic);
    endfor
    let outputs = {};
    for destination in (destinations)
      const {connection, index} = destination;
      const rendered = batch['results][index];
      const mime = tosym(strsub(tostr(rendered['content_type]), "_", "/", 1));
      if (length(rendered['body]))
        outputs = {@outputs, {connection, mime, rendered['body], rendered['annotations]}};
      endif
    endfor
    return ['document -> batch['document], 'connections -> outputs,
      'history -> history ? batch['results][1] | false];
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

  method _capture_room owner: ARCH_WIZARD
    "Capture room content under this viewer's permissions, including producer callbacks.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const {room} = args;
    return room:look_self():capture(this);
  endmethod

  method emit_room_look owner: ARCH_WIZARD
    "Capture once, then deliver narrative and current panel state from the same LookView.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {?room = this.location} = args;
    valid(room) && isa(room, $room) || return;
    this.location == room || return;
    "An offline player's arrival has neither a live panel nor a narrative recipient.";
    if (is_player(this) && !length(this:_connections()))
      return false;
    endif
    const view = this:_capture_room(room);
    const event = $look:narrative(view):with_metadata('look_kind, "room")
      :with_metadata('look_room, room);
    this.location == room || return;
    this:emit_room_snapshot_state(this, room, view);
    this:inform_current(event);
  endmethod

  method emit_room_snapshot_state owner: ARCH_WIZARD
    "Publish live panel state for this receiver; stale captures cannot replace the current room.";
    this:_can_inform_as(caller, caller_perms()) || raise(E_PERM);
    const {viewer, room, ?captured = false} = args;
    viewer == this || raise(E_PERM);
    is_player(this) && valid(room) && isa(room, $room) || return;
    this.location == room || return;
    "Panel state is transient; disconnected players need no capture or rendering.";
    length(this:_connections()) || return false;
    const view = typeof(captured) == TYPE_MAP ? captured | this:_capture_room(room);
    view['viewer] == this && view['room_ref] == room || raise(E_INVARG, "Room view mismatch");
    view['exit_revision] == room.exit_revision || return false;
    const snapshot = $look:panel(view);
    this.location == room || return;
    view['exit_revision] == room.exit_revision || return false;
    emit_data(this, 'state, 'room_snapshot, snapshot);
    return true;
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
    $test_utils:assert_raises(E_PERM, receiver, "inform_all", {event}, "inform_all should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "_connections", {}, "_connections should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "rewrite_event", {"rewrite-id", "replacement"}, "rewrite_event should reject unrelated callers");
    $test_utils:assert_raises(E_PERM, receiver, "emit_room_snapshot_state", {receiver, $prototype_box}, "emit_room_snapshot_state should reject unrelated callers");
    return true;
  endmethod
endobject
