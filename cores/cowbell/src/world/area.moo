object AREA [
  import_export_id -> "area",
  import_export_hierarchy -> {"world"}
]
  name: "Generic Area"
  parent: ROOT
  owner: HACKER
  fertile: true
  readable: true

  property passages_rel (owner: HACKER, flags: "r") = 0;

  override description (owner: HACKER, flags: "rc") = "Area container that manages passages between rooms using a relation.";

  method acceptable owner: ARCH_WIZARD
    "Native movement requires the same area admission authority as add_room.";
    const actor = caller_perms();
    set_task_perms(actor);
    const {what} = args;
    typeof(what) == TYPE_OBJ && valid(what) && isa(what, $room) || return false;
    const grant = `$root:_check_permissions_as($area:_subject_for(actor, this, 'area), actor, 'add_room)
      ! E_PERM => {}';
    if (length(grant))
      return true;
    endif
    "An unstored bearer remains available on the checked movement activation.";
    for frame in (callers())
      if (frame[4] == $area && frame[2] == "_move_room_here" && frame[3] == actor
          && typeof(frame[1]) == TYPE_FLYWEIGHT && frame[1].delegate == this)
        return `$root:_check_permissions_as(frame[1], actor, 'add_room) ! E_PERM => false';
      endif
    endfor
    return false;
  endmethod

  method initialize owner: ARCH_WIZARD
    "Allocate the owned passage relation only for an authorized area lifecycle caller.";
    const actor = caller_perms();
    actor == this.owner || (valid(actor) && actor.wizard) || raise(E_PERM);
    pass();
    this:_ensure_passages_relation();
  endmethod

  method _ensure_passages_relation owner: ARCH_WIZARD
    "Create the area's protected relation under its owner after checking caller authority.";
    const actor = caller_perms();
    actor == this.owner || (valid(actor) && actor.wizard) || raise(E_PERM);
    if (typeof(this.passages_rel) == TYPE_OBJ && valid(this.passages_rel))
      return this.passages_rel;
    endif
    const relation = create($relation, $arch_wizard);
    relation.name = "Passages Relation for Area " + tostr(this);
    relation.r = 1;
    this.passages_rel = relation;
    return relation;
  endmethod

  method _canonical_tuple owner: HACKER
    "Validate the endpoints and build {min_room, max_room, passage} for storage.";
    const {room_a, room_b, passage} = args;
    typeof(room_a) == TYPE_OBJ && typeof(room_b) == TYPE_OBJ || raise(E_TYPE);
    valid(room_a) && valid(room_b) || raise(E_INVARG, "A passage needs two existing rooms.");
    isa(room_a, $room) && isa(room_b, $room) || raise(E_TYPE, "Passage endpoints must be rooms.");
    room_a != room_b || raise(E_INVARG, "A passage must connect two different rooms.");
    typeof(passage) == TYPE_OBJ || typeof(passage) == TYPE_FLYWEIGHT || raise(E_TYPE);
    const prototype = typeof(passage) == TYPE_FLYWEIGHT ? passage.delegate | passage;
    valid(prototype) && isa(prototype, $passage) || raise(E_TYPE, "Expected a passage.");
    ((passage.side_a_room == room_a && passage.side_b_room == room_b)
      || (passage.side_a_room == room_b && passage.side_b_room == room_a))
      || raise(E_INVARG, "Passage endpoints do not match the connection.");
    if (room_a < room_b)
      return {room_a, room_b, passage};
    else
      return {room_b, room_a, passage};
    endif
  endmethod

  method passage_for owner: ARCH_WIZARD
    "Find one connection between two rooms; parallel connections require an identity.";
    set_task_perms(caller_perms());
    const {room_a, room_b, ?identity = ""} = args;
    return $room:passage_to(room_a, room_b, identity);
  endmethod

  method _passage_tuple owner: ARCH_WIZARD
    "Read an exact endpoint pair and optional connection identity from one area's store.";
    set_task_perms(caller_perms());
    const {area, room_a, room_b, ?identity = ""} = args;
    typeof(room_a) == TYPE_OBJ && typeof(room_b) == TYPE_OBJ || raise(E_TYPE);
    room_a == room_b && return {};
    let found = {};
    for passage in ($area:stored_passages_from(area, room_a))
      if (passage.side_a_room != room_b && passage.side_b_room != room_b)
        continue;
      endif
      if (identity != "" && passage.connection_id != identity)
        continue;
      endif
      length(found) && raise(E_INVARG, "Several passages connect those rooms. Choose a direction or connection identity.");
      found = $area:_canonical_tuple(room_a, room_b, passage);
    endfor
    return found;
  endmethod

  method passage_link_id owner: ARCH_WIZARD
    "Return a connection's identity; a room pair alone must select exactly one passage.";
    set_task_perms(caller_perms());
    const {room_a, room_b, ?identity = ""} = args;
    const passage = $room:passage_to(room_a, room_b, identity);
    return typeof(passage) == TYPE_FLYWEIGHT ? passage.connection_id | "";
  endmethod

  method set_passage owner: ARCH_WIZARD
    "Install or replace the sole connection between two rooms; replacement gets a new identity.";
    const actor = caller_perms();
    const {room_a, room_b, passage} = args;
    $root:_check_permissions_as(this, actor, 'create_passage);
    $area:_check_endpoints(actor, room_a, room_b, passage);
    const source = typeof(room_a) == TYPE_FLYWEIGHT ? room_a.delegate | room_a;
    const destination = typeof(room_b) == TYPE_FLYWEIGHT ? room_b.delegate | room_b;
    const old = $area:_passage_tuple(this, source, destination);
    return this:_do_create_passage(source, destination, passage, length(old) ? old[3] | false, false);
  endmethod

  method clear_passage owner: ARCH_WIZARD
    "Remove one connection with area and source-room authority.";
    set_task_perms(caller_perms());
    return this:remove_passage(@args);
  endmethod

  method passages owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    "Return all passage objects.";
    if (typeof(this.passages_rel) != TYPE_OBJ || !valid(this.passages_rel))
      return {};
    endif
    tuples = this.passages_rel:tuples();
    return { tuple[3] for tuple in (tuples) };
  endmethod

  method passages_from owner: ARCH_WIZARD
    "Return all connections at a room, including those registered in other areas.";
    set_task_perms(caller_perms());
    const {room} = args;
    return $room:passages(room);
  endmethod

  method handle_passage_command owner: ARCH_WIZARD
    "Handle a matched direction; command recognition is separate from movement success.";
    const actor = caller_perms();
    actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(actor);
    const {parsed} = args;
    const room = player.location;
    valid(room) && isa(room, $room) || return false;
    const command = tostr(parsed['verb]) == "go" ? parsed['dobjstr] | tostr(parsed['verb]);
    const passage = $room:find_passage(room, command);
    typeof(passage) == TYPE_FLYWEIGHT || return false;
    passage:travel_from(player, room, parsed);
    return true;
  endmethod

  method rooms_from owner: ARCH_WIZARD
    "Find rooms reachable through enabled passage sides, across area boundaries.";
    set_task_perms(caller_perms());
    const {start_room, ?only_open = true} = args;
    let visited = {start_room};
    let frontier = {start_room};
    while (length(frontier))
      suspend_if_needed();
      const current = frontier[1];
      frontier = listdelete(frontier, 1);
      for passage in ($room:passages(current))
        if (!$passage:allows_direction(passage, current) || (only_open && !passage.is_open))
          continue;
        endif
        const next = passage:other_room(current);
        if (!(next in visited))
          visited = {@visited, next};
          frontier = {@frontier, next};
        endif
      endfor
    endwhile
    return visited;
  endmethod

  method connected owner: HACKER
    "Check if two rooms are transitively connected via passages.";
    "Optional third arg: only_open (default true) - skip closed passages.";
    {room_a, room_b, ?only_open = true} = args;
    typeof(room_a) == TYPE_OBJ && typeof(room_b) == TYPE_OBJ || raise(E_TYPE);
    reachable = this:rooms_from(room_a, only_open);
    return room_b in reachable;
  endmethod

  method find_path owner: ARCH_WIZARD
    "Find a route through each room's connections, independent of area membership.";
    "Return {room, connector} steps; transport steps remain explicit.";
    set_task_perms(caller_perms());
    const {start_room, goal_room, ?only_open = true, ?include_transports = true} = args;
    typeof(start_room) == TYPE_OBJ && typeof(goal_room) == TYPE_OBJ || raise(E_TYPE);
    valid(start_room) && valid(goal_room) || return false;
    start_room == goal_room && return {{start_room, false}};
    let visited = [start_room -> true];
    let queue = {{start_room, {}}};
    while (length(queue))
      suspend_if_needed(50000);
      const {current, path} = queue[1];
      queue = listdelete(queue, 1);
      let edges = {};
      for passage in ($room:passages(current))
        if ($passage:allows_direction(passage, current) && (!only_open || passage.is_open))
          edges = {@edges, {passage:other_room(current), passage}};
        endif
      endfor
      if (include_transports)
        for connection in (current:transport_destinations())
          const {next, label, provider} = connection;
          edges = {@edges, {next, {'transport, label, provider}}};
        endfor
      endif
      for edge in (edges)
        const {next, connector} = edge;
        if (!valid(next) || !isa(next, $room))
          continue;
        endif
        if (next == goal_room)
          return {@path, {current, connector}, {goal_room, false}};
        endif
        if (!maphaskey(visited, next))
          visited[next] = true;
          queue = {@queue, {next, {@path, {current, connector}}}};
        endif
      endfor
    endwhile
    return false;
  endmethod

  method _move_room_here owner: ARCH_WIZARD
    "Validate both admission and room movement, then perform the native move as the principal.";
    const actor = caller_perms();
    const {subject} = args;
    const {area, _} = $root:_check_permissions_as(this, actor, 'add_room);
    const {room, principal, grants} = $root:_check_permissions_with_grants_as(subject, actor, 'move);
    valid(room) && isa(room, $room) || raise(E_TYPE);
    set_task_perms(principal, grants);
    move(room, area);
    room.location == area || raise(E_INVARG, "Could not move room into area.");
    return room;
  endmethod

  method make_room_in owner: ARCH_WIZARD
    "Create a new room in this area. Requires 'add_room capability on area.";
    const actor = caller_perms();
    $root:_check_permissions_as(this, actor, 'add_room);
    const {parent_obj} = args;
    typeof(parent_obj) == TYPE_OBJ && valid(parent_obj) && isa(parent_obj, $room) || raise(E_TYPE);
    "Create room with caller's ownership";
    set_task_perms(actor);
    const new_room = parent_obj:create();
    try
      return this:_move_room_here(new_room);
    except e (ANY)
      valid(new_room) && new_room:destroy();
      raise(e[1], length(e) >= 2 ? e[2] | "Could not create room in area.");
    endtry
  endmethod

  method add_room owner: ARCH_WIZARD
    "Add a room to this area. Requires 'add_room capability on area.";
    set_task_perms(caller_perms());
    const {room} = args;
    return this:_move_room_here(room);
  endmethod

  method create_passage owner: ARCH_WIZARD
    "Create a connection after checking the storing area and both endpoints.";
    const actor = caller_perms();
    const {room_a, room_b, passage, ?allow_parallel = false} = args;
    const {area, _} = $root:_check_permissions_as(this, actor, 'create_passage);
    $area:_check_endpoints(actor, room_a, room_b, passage);
    const source = typeof(room_a) == TYPE_FLYWEIGHT ? room_a.delegate | room_a;
    const destination = typeof(room_b) == TYPE_FLYWEIGHT ? room_b.delegate | room_b;
    if (!allow_parallel)
      for existing in ($room:passages(source))
        existing:other_room(source) != destination
          || raise(E_INVARG, "Those rooms are already connected. Choose parallel mode to add another passage.");
      endfor
    endif
    return area:_do_create_passage(source, destination, passage);
  endmethod

  method _do_create_passage owner: ARCH_WIZARD
    "Write one checked connection and index its store at both endpoints.";
    const principal = caller_perms();
    valid(principal) && principal.wizard || raise(E_PERM);
    const {room_a, room_b, proposed, ?previous = false, ?keep_identity = false} = args;
    $area:_canonical_tuple(room_a, room_b, proposed);
    const replacing = typeof(previous) == TYPE_FLYWEIGHT;
    if (replacing)
      const registered = $area:_passage_tuple(this, room_a, room_b, previous.connection_id);
      length(registered) && equal(registered[3], previous)
        || raise(E_INVARG, "The passage changed. Read it again before editing.");
    endif
    const prototype = typeof(proposed) == TYPE_FLYWEIGHT ? proposed.delegate | proposed;
    let passage = typeof(proposed) == TYPE_FLYWEIGHT ? proposed
      | toflyweight(prototype, $passage:configuration(proposed), {});
    if (replacing && keep_identity)
      passage.connection_id = previous.connection_id;
    else
      passage.connection_id = uuid();
    endif
    for room in ({room_a, room_b})
      const names = $passage:command_names(passage, room);
      for existing in ($room:passages(room))
        if (replacing && existing.connection_id == previous.connection_id)
          continue;
        endif
        passage.connection_id != existing.connection_id
          || raise(E_INVARG, "That connection identity is already installed.");
        const existing_names = $passage:expand_direction_aliases($passage:command_names(existing, room));
        for alias in ($passage:expand_direction_aliases(names))
          !(alias in existing_names)
            || raise(E_INVARG, "Exit name '" + alias + "' is already used at " + tostr(room) + ".");
        endfor
      endfor
    endfor
    const tuple = $area:_canonical_tuple(room_a, room_b, passage);
    this:_ensure_passages_relation();
    if (replacing)
      this.passages_rel:retract($area:_canonical_tuple(room_a, room_b, previous));
    endif
    this.passages_rel:assert(tuple);
    for room in ({room_a, room_b})
      room.passage_areas = setadd(room.passage_areas, this);
    endfor
    $room:connections_changed({room_a, room_b});
    return passage;
  endmethod

  method update_passage owner: ARCH_WIZARD
    "Edit a named connection; changing shared fields requires authority over both sides.";
    const actor = caller_perms();
    const {source_room, dest_room, proposed} = args;
    const source = typeof(source_room) == TYPE_FLYWEIGHT ? source_room.delegate | source_room;
    const destination = typeof(dest_room) == TYPE_FLYWEIGHT ? dest_room.delegate | dest_room;
    $area:_canonical_tuple(source, destination, proposed);
    $area:_check_room_right(actor, source_room, 'dig_from);
    const registered = $room:find_connection(source, proposed.connection_id);
    length(registered) || raise(E_INVARG, "No matching connection. Read the current passage before editing.");
    const area = registered[1];
    const tuple = $area:_passage_tuple(area, source, destination, proposed.connection_id);
    length(tuple) || raise(E_INVARG, "No matching connection. Read the current passage before editing.");
    const previous = tuple[3];
    const prototype = typeof(proposed) == TYPE_FLYWEIGHT ? proposed.delegate | proposed;
    let before = $passage:configuration(previous);
    let after = $passage:configuration(proposed);
    const prefix = previous.side_a_room == source ? "side_a_" | "side_b_";
    for field in ({"label", "aliases", "description", "description_kind", "ambient", "leave_msg", "arrive_msg",
        "prose_style", "departure_phrase", "arrival_phrase"})
      const key = tosym(prefix + field);
      before = `mapdelete(before, key) ! E_RANGE => before';
      after = `mapdelete(after, key) ! E_RANGE => after';
    endfor
    const same_behavior = previous.delegate == prototype
      && equal(flycontents(previous), typeof(proposed) == TYPE_FLYWEIGHT ? flycontents(proposed) | {});
    if (!same_behavior || !equal(before, after))
      $area:_check_room_right(actor, dest_room, 'dig_from);
    endif
    if (previous.side_a_enabled != proposed.side_a_enabled || previous.side_b_enabled != proposed.side_b_enabled)
      $area:_check_endpoints(actor, source_room, dest_room, proposed);
    endif
    return area:_do_create_passage(source, destination, proposed, previous, true);
  endmethod

  method remove_passage owner: ARCH_WIZARD
    "An endpoint owner can disconnect it; delegates also need authority over the store.";
    const actor = caller_perms();
    const {room_a, room_b, ?identity = ""} = args;
    $area:_check_room_right(actor, room_a, 'dig_from);
    const source = typeof(room_a) == TYPE_FLYWEIGHT ? room_a.delegate | room_a;
    const destination = typeof(room_b) == TYPE_FLYWEIGHT ? room_b.delegate | room_b;
    source != destination || raise(E_INVARG, "A passage must connect two different rooms.");
    const passage = $room:passage_to(source, destination, identity);
    typeof(passage) == TYPE_FLYWEIGHT || return false;
    const area = $room:find_connection(source, passage.connection_id)[1];
    const receiver = typeof(this) == TYPE_FLYWEIGHT ? this.delegate | this;
    const subject = receiver == area ? this | $area:_subject_for(actor, area, 'area);
    if (actor != source.owner)
      $root:_check_permissions_as(subject, actor, 'remove_passage);
    endif
    return area:_do_remove_passage(source, destination, passage.connection_id);
  endmethod

  method _do_remove_passage owner: ARCH_WIZARD
    "Remove a checked connection and remove empty endpoint indexes.";
    const principal = caller_perms();
    valid(principal) && principal.wizard || raise(E_PERM);
    const {room_a, room_b, ?identity = ""} = args;
    room_a != room_b || raise(E_INVARG, "A passage must connect two different rooms.");
    const tuple = $area:_passage_tuple(this, room_a, room_b, identity);
    length(tuple) || return false;
    this.passages_rel:retract(tuple);
    for room in ({room_a, room_b})
      if (valid(room) && !length($area:stored_passages_from(this, room)))
        room.passage_areas = setremove(room.passage_areas, this);
      endif
    endfor
    $room:connections_changed({room_a, room_b});
    return true;
  endmethod

  method on_room_recycle owner: ARCH_WIZARD
    "Remove an endpoint's connections even after that room moves to another area.";
    const actor = caller_perms();
    const {room} = args;
    valid(room) && isa(room, $room) || raise(E_TYPE);
    actor == room.owner || (valid(actor) && actor.wizard) || raise(E_PERM);
    for passage in ($area:stored_passages_from(this, room))
      this:_do_remove_passage(passage.side_a_room, passage.side_b_room, passage.connection_id);
    endfor
  endmethod

  method find_passage_by_direction owner: ARCH_WIZARD
    "Find one matching connection at either area's endpoint, rejecting ambiguous names.";
    set_task_perms(caller_perms());
    return $room:find_passage(@args);
  endmethod

  method get_exit_info owner: ARCH_WIZARD
    "Return {plain labels, ambient tuples} from the room's common exit query.";
    "Ambient tuples contain {inline description, prose style, label, destination}.";
    set_task_perms(caller_perms());
    const {room, ?viewer = player} = args;
    const options = $room:exit_options(room, viewer);
    return {{option['label] for option in (options) if (!option['ambient])},
      {{ambient['description], ambient['prose_style], ambient['label], ambient['destination]}
        for ambient in (options) if (ambient['ambient])}};
  endmethod

  method test_connectivity owner: HACKER
    "Test connectivity checking between rooms";
    area = $area:create(true);
    r1 = $room:create(true);
    r2 = $room:create(true);
    r3 = $room:create(true);
    r4 = $room:create(true);
    "Create three rooms in a chain: r1 <-> r2 <-> r3";
    passage1 = <$passage, .side_a_room = r1, .side_b_room = r2, .is_open = true>;
    passage2 = <$passage, .side_a_room = r2, .side_b_room = r3, .is_open = true>;
    passage1 = area:set_passage(r1, r2, passage1);
    passage2 = area:set_passage(r2, r3, passage2);
    "r1 and r2 are directly connected";
    !area:connected(r1, r2) && raise(E_ASSERT, "r1 and r2 should be connected");
    "r1 and r3 are transitively connected";
    !area:connected(r1, r3) && raise(E_ASSERT, "r1 and r3 should be transitively connected");
    "r3 and r1 are connected (bidirectional)";
    !area:connected(r3, r1) && raise(E_ASSERT, "r3 and r1 should be connected");
    "r4 is not connected";
    area:connected(r1, r4) && raise(E_ASSERT, "r4 should not be connected");
  endmethod

  method test_rooms_from owner: HACKER
    "Test finding all reachable rooms";
    area = $area:create(true);
    r1 = $room:create(true);
    r2 = $room:create(true);
    r3 = $room:create(true);
    r4 = $room:create(true);
    "Create a small network";
    area:set_passage(r1, r2, <$passage, .side_a_room = r1, .side_b_room = r2, .is_open = true>);
    area:set_passage(r2, r3, <$passage, .side_a_room = r2, .side_b_room = r3, .is_open = true>);
    area:set_passage(r1, r4, <$passage, .side_a_room = r1, .side_b_room = r4, .is_open = true>);
    reachable = area:rooms_from(r1);
    length(reachable) != 4 && raise(E_ASSERT, "Should find 4 reachable rooms from r1");
    r1 in reachable || raise(E_ASSERT, "Should include starting room");
    r2 in reachable || raise(E_ASSERT, "Should reach r2");
    r3 in reachable || raise(E_ASSERT, "Should reach r3");
    r4 in reachable || raise(E_ASSERT, "Should reach r4");
    "From r3 should reach all via bidirectional edges";
    reachable = area:rooms_from(r3);
    length(reachable) != 4 && raise(E_ASSERT, "Should find 4 reachable rooms from r3");
  endmethod

  method test_find_path owner: HACKER
    "Test pathfinding between rooms";
    area = $area:create(true);
    r1 = $room:create(true);
    r2 = $room:create(true);
    r3 = $room:create(true);
    r_disconnected = $room:create(true);
    p1 = <$passage, .side_a_room = r1, .side_b_room = r2, .is_open = true>;
    p2 = <$passage, .side_a_room = r2, .side_b_room = r3, .is_open = true>;
    "Create chain: r1 <-> r2 <-> r3";
    p1 = area:set_passage(r1, r2, p1);
    p2 = area:set_passage(r2, r3, p2);
    "Direct path";
    path = area:find_path(r1, r2);
    path || raise(E_ASSERT, "Should find path from r1 to r2");
    length(path) != 2 && raise(E_ASSERT, "Path length should be 2");
    path[1][1] != r1 && raise(E_ASSERT, "Path should start at r1");
    path[2][1] != r2 && raise(E_ASSERT, "Path should end at r2");
    "Multi-hop path";
    path = area:find_path(r1, r3);
    path || raise(E_ASSERT, "Should find path from r1 to r3");
    length(path) != 3 && raise(E_ASSERT, "Path should have 3 nodes");
    path[1][1] != r1 && raise(E_ASSERT, "Path should start at r1");
    path[2][1] != r2 && raise(E_ASSERT, "Path should go through r2");
    path[3][1] != r3 && raise(E_ASSERT, "Path should end at r3");
    "No path to disconnected room";
    path = area:find_path(r1, r_disconnected);
    path && raise(E_ASSERT, "Should not find path to disconnected room");
    "Same room";
    path = area:find_path(r1, r1);
    path || raise(E_ASSERT, "Should find path from room to itself");
    length(path) != 1 && raise(E_ASSERT, "Same-room path should have 1 node");
  endmethod

  method test_closed_passages owner: HACKER
    "Test that closed passages are respected by pathfinding";
    area = $area:create(true);
    r1 = $room:create(true);
    r2 = $room:create(true);
    r3 = $room:create(true);
    "Create chain with middle passage closed: r1 <-> r2 -X- r3";
    area:set_passage(r1, r2, <$passage, .side_a_room = r1, .side_b_room = r2, .is_open = true>);
    area:set_passage(r2, r3, <$passage, .side_a_room = r2, .side_b_room = r3, .is_open = false>);
    "r1 to r3 should fail with only_open=true (default)";
    path = area:find_path(r1, r3);
    path && raise(E_ASSERT, "Should not find path through closed passage");
    "r1 to r3 should succeed with only_open=false";
    path = area:find_path(r1, r3, false);
    path || raise(E_ASSERT, "Should find path when ignoring closed passages");
    length(path) != 3 && raise(E_ASSERT, "Path should have 3 nodes");
    "rooms_from should respect is_open";
    reachable = area:rooms_from(r1);
    length(reachable) != 2 && raise(E_ASSERT, "Should only reach 2 rooms with closed passage");
    reachable = area:rooms_from(r1, false);
    length(reachable) != 3 && raise(E_ASSERT, "Should reach all 3 rooms when ignoring closed");
  endmethod

  method test_accepts_rooms owner: HACKER
    "Test that areas accept and add rooms as spatial contents, while rejecting non-rooms.";
    area = $area:create(true);
    room = $room:create(true);
    added = $room:create(true);
    made = #-1;
    thing = $thing:create(true);
    try
      move(room, area);
      $test_utils:assert_eq(room.location, area, "raw move should put rooms in area");
      $test_utils:assert_true(room in area:contents(), "raw move should add room to area contents");
      area:add_room(added);
      $test_utils:assert_eq(added.location, area, "add_room should put rooms in area");
      $test_utils:assert_true(added in area:contents(), "add_room should add room to area contents");
      made = area:make_room_in($room);
      $test_utils:assert_true(valid(made), "make_room_in should create a room");
      $test_utils:assert_eq(parent(made), $room, "make_room_in should use the requested room parent");
      $test_utils:assert_eq(made.location, area, "make_room_in should put created room in area");
      $test_utils:assert_true(made in area:contents(), "make_room_in should add created room to area contents");
      accepted_thing = true;
      try
        move(thing, area);
      except (E_NACC)
        accepted_thing = false;
      endtry
      $test_utils:assert_false(accepted_thing, "raw move should reject non-room contents");
      $test_utils:assert_raises(E_TYPE, area, "add_room", {thing}, "add_room should reject non-room contents");
      $test_utils:assert_raises(E_TYPE, area, "make_room_in", {$thing}, "make_room_in should reject non-room parents");
    finally
      $test_utils:destroy_if_valid(made);
      $test_utils:destroy_if_valid(thing);
      $test_utils:destroy_if_valid(added);
      $test_utils:destroy_if_valid(room);
      $test_utils:destroy_if_valid(area);
    endtry
  endmethod

  method destroy owner: ARCH_WIZARD
    "Check recycle authority before removing the area's connections and private relation.";
    const actor = caller_perms();
    const {target, _, __} = $root:_check_permissions_with_grants_as(this, actor, 'recycle);
    for passage in (target:passages())
      target:_do_remove_passage(passage.side_a_room, passage.side_b_room, passage.connection_id);
    endfor
    if (typeof(target.passages_rel) == TYPE_OBJ && valid(target.passages_rel))
      target.passages_rel:destroy();
    endif
    set_task_perms(actor);
    return pass();
  endmethod

  method stored_passages_from owner: ARCH_WIZARD
    "Read one area's relation without invoking callbacks on a supplied area or relation.";
    set_task_perms(caller_perms());
    const {area, room} = args;
    valid(area) && isa(area, $area) || raise(E_TYPE);
    const relation = area.passages_rel;
    typeof(relation) == TYPE_OBJ && valid(relation) || return {};
    isa(relation, $relation) || raise(E_INVARG, "Invalid passage relation.");
    let passages = {};
    for tuple in ($relation:select_containing(room, relation))
      if (tuple[1] == room || tuple[2] == room)
        passages = {@passages, tuple[3]};
      endif
    endfor
    return passages;
  endmethod

  method _check_room_right owner: ARCH_WIZARD
    "Validate a supplied room capability or the principal's stored room grant.";
    const {actor, subject, right} = args;
    const target = typeof(subject) == TYPE_FLYWEIGHT ? subject | $area:_subject_for(actor, subject, 'room);
    return $root:_check_permissions_as(target, actor, right);
  endmethod

  method _subject_for owner: ARCH_WIZARD
    "Look up a stored grant under its holder's authority and check the target binding.";
    const {actor, target, category} = args;
    set_task_perms(actor);
    const capability = actor:find_capability_for(target, category);
    if (typeof(capability) == TYPE_FLYWEIGHT)
      capability.delegate == target || raise(E_PERM, "The capability names a different object.");
      return capability;
    endif
    return target;
  endmethod

  method _check_endpoints owner: ARCH_WIZARD
    "Check endpoint identity and the rights needed for each enabled direction.";
    const {actor, room_a, room_b, passage} = args;
    const source = typeof(room_a) == TYPE_FLYWEIGHT ? room_a.delegate | room_a;
    const destination = typeof(room_b) == TYPE_FLYWEIGHT ? room_b.delegate | room_b;
    $area:_canonical_tuple(source, destination, passage);
    $area:_check_room_right(actor, room_a, 'dig_from);
    $area:_check_room_right(actor, room_b, 'dig_into);
    if ($passage:allows_direction(passage, destination))
      $area:_check_room_right(actor, room_b, 'dig_from);
      $area:_check_room_right(actor, room_a, 'dig_into);
    endif
  endmethod

  method rebuild_passages owner: ARCH_WIZARD
    "Upgrade an old area's passage values and endpoint indexes; return the old relation as a backup.";
    caller_perms().wizard || raise(E_PERM);
    const previous_relation = this.passages_rel;
    typeof(previous_relation) == TYPE_OBJ && valid(previous_relation) || return #-1;
    let prepared = {};
    let names = [];
    for tuple in ($relation:tuples(previous_relation))
      const {source, destination, old} = tuple;
      $area:_canonical_tuple(source, destination, old);
      const prototype = typeof(old) == TYPE_FLYWEIGHT ? old.delegate | old;
      const contents = typeof(old) == TYPE_FLYWEIGHT ? flycontents(old) | {};
      let passage = toflyweight(prototype, $passage:configuration(old), contents);
      passage.connection_id = uuid();
      for side in ({"a", "b"})
        const prefix = "side_" + side + "_";
        if (typeof(old) == TYPE_FLYWEIGHT && !maphaskey(flyslots(old), tosym(prefix + "enabled")))
          passage.(prefix + "enabled") = passage.(prefix + "label") != ""
            || length(passage.(prefix + "aliases")) > 0;
        endif
      endfor
      for room in ({source, destination})
        let used = `names[room] ! E_RANGE => {}';
        for name in ($passage:command_names(passage, room))
          const aliases = $passage:expand_direction_aliases({name});
          for alias in (aliases)
            !(alias in used) || raise(E_INVARG, "Conflicting exit name '" + alias + "' at " + tostr(room) + ".");
          endfor
        endfor
        names[room] = {@used, @$passage:expand_direction_aliases($passage:command_names(passage, room))};
      endfor
      prepared = {@prepared, $area:_canonical_tuple(source, destination, passage)};
    endfor
    "Finish validation before replacing the live store. Retain the old relation for inspection.";
    const relation = create($relation, $arch_wizard);
    relation.r = 1;
    relation.name = "Passages Relation for Area " + tostr(this);
    for tuple in (prepared)
      relation:assert(tuple);
    endfor
    this.passages_rel = relation;
    for tuple in (prepared)
      for room in ({tuple[1], tuple[2]})
        room.passage_areas = setadd(room.passage_areas, this);
      endfor
    endfor
    $room:connections_changed(mapkeys(names));
    return previous_relation;
  endmethod
endobject
