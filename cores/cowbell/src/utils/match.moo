object MATCH [
  import_export_id -> "match",
  import_export_hierarchy -> {"utils"}
]
  name: "Object Matching Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Resolve object references and scoped names, including aliases and ordinal selections.";

  method match_object owner: ARCH_WIZARD
    "Match an object reference string to an actual object.";
    "Handles: #123, #UUID-style, $system.prop, @playername, or plain name matching.";
    "Usage: $match:match_object(ref_string [, context_object])";
    "The context_object (defaults to player) determines:";
    "  - What 'me' and 'player' resolve to (the context_object itself)";
    "  - What 'here' resolves to (context_object.location)";
    "  - Which visible scope supplies names and aliases";
    set_task_perms(caller_perms());
    let {ref_string, ?context = player} = args;
    typeof(ref_string) != TYPE_STR && raise(E_TYPE, "Object reference must be a string");
    ref_string = ref_string:reference_text();
    !ref_string && raise(E_INVARG, "Empty object reference");
    if (ref_string[1] == "#")
      const result = toobj(ref_string);
      if (result == #0 && ref_string != "#0")
        raise(E_INVARG, "Invalid object reference: " + ref_string);
      endif
      valid(result) || raise(E_INVARG, "Object " + ref_string + " does not exist");
      return result;
    elseif (ref_string[1] == "$")
      let prop_path = ref_string[2..$];
      prop_path || raise(E_INVARG, "Empty system reference after $");
      try
        let current_obj = #0;
        for prop_name in (prop_path:split("."))
          current_obj = current_obj.(prop_name);
        endfor
        typeof(current_obj) == TYPE_OBJ || raise(E_TYPE, "System reference $" + prop_path + " is not an object");
        return current_obj;
      except e (E_PROPNF)
        raise(E_PROPNF, "System property $" + prop_path + " does not exist");
      except e (ANY)
        raise(E_INVARG, "Invalid system reference $" + prop_path);
      endtry
    elseif (ref_string[1] == "@")
      let player_name = ref_string[2..$];
      player_name || raise(E_INVARG, "Empty player reference after @");
      return this:match_player(player_name, context);
    else
      return this:match_by_name(ref_string, context);
    endif
  endmethod

  method match_player owner: ARCH_WIZARD
    "Match player by name or object number using complex_match builtin.";
    set_task_perms(caller_perms());
    const {player_name, ?context = player} = args;
    player_name || raise(E_INVARG, "Empty player name");
    "Handle 'me'/'myself' as special case";
    if (player_name in {"me", "myself"})
      valid(context) && is_player(context) && return context;
      raise(E_INVARG, "No player context for 'me'");
    endif
    "Handle object number references (e.g., '#2', '#000053-9A6FBE399A')";
    if (player_name[1] == "#")
      const obj = `toobj(player_name) ! E_RANGE => $nothing';
      !valid(obj) && raise(E_INVARG, "Invalid object reference: " + player_name);
      !is_player(obj) && raise(E_INVARG, tostr(obj) + " is not a player.");
      return obj;
    endif
    return this:require_unique(player_name, this:candidates_in_scope(player_name, players()), "player");
  endmethod

  method match_by_name owner: ARCH_WIZARD
    "Resolve a name using the command scope, including aliases, worn items, and scope hooks.";
    set_task_perms(caller_perms());
    const {name_string, ?context = player} = args;
    if (name_string == "here")
      valid(context) && valid(context.location) && return context.location;
      raise(E_INVARG, "No location to match 'here'");
    elseif (name_string in {"me", "player"})
      valid(context) && return context;
      raise(E_INVARG, "No context to match '" + name_string + "'");
    endif
    return this:require_unique(name_string, this:candidates_in_scope(name_string, this:environment(context)));
  endmethod

  method normalize_scope owner: ARCH_WIZARD
    "Return {objects, keys} for objects or {object, names...} scope entries.";
    "Preserve first occurrence order. Explicit scope names replace default name/aliases.";
    set_task_perms(caller_perms());
    const {scope} = args;
    let objects = {};
    let keys = {};
    let positions = [];
    let explicit = [];
    for entry in (scope)
      const object = typeof(entry) == TYPE_LIST && entry ? entry[1] | entry;
      if (typeof(object) != TYPE_OBJ || !valid(object))
        continue;
      endif
      if (!maphaskey(positions, object))
        objects = {@objects, object};
        positions[object] = length(objects);
        keys = {@keys, {}};
      endif
      if (typeof(entry) == TYPE_LIST && length(entry) > 1)
        const source_names = entry[2..$];
        const names = {name for name in (source_names) if typeof(name) == TYPE_STR && length(name) > 0};
        if (names)
          const position = positions[object];
          for name in (names)
            keys[position] = setadd(keys[position], name);
          endfor
          explicit[object] = true;
        endif
      endif
    endfor
    for object in (objects)
      if (!maphaskey(explicit, object))
        const aliases = `object.aliases ! E_PROPNF, E_PERM => {}';
        keys[positions[object]] = {object.name, @{name for name in (aliases) if typeof(name) == TYPE_STR && length(name) > 0}};
      endif
    endfor
    return {objects, keys};
  endmethod

  method candidates_in_scope owner: ARCH_WIZARD
    "Return all best matches in scope, or the ordinal selection. IDs must belong to scope.";
    set_task_perms(caller_perms());
    const {token, scope, ?options = []} = args;
    const {objects, keys} = this:normalize_scope(scope);
    const text = token:trim();
    !text && return {};
    if (`options['allow_literals] ! E_RANGE => true')
      const literal = text:match_objid();
      if (literal && literal['start] == 1 && literal['end] == length(text))
        const object = `toobj(text) ! E_RANGE => $nothing';
        return object in objects ? {object} | {};
      endif
    endif
    const threshold = `options['fuzzy_threshold] ! E_RANGE => 0.3';
    return complex_matches(text, objects, keys, typeof(threshold) == TYPE_BOOL ? (threshold ? 0.3 | 0.0) | tofloat(threshold), false);
  endmethod

  method resolve_in_scope owner: ARCH_WIZARD
    "Return one scoped match, $failed_match, or $ambiguous_match.";
    set_task_perms(caller_perms());
    const matches = this:candidates_in_scope(@args);
    return !matches ? $failed_match | length(matches) == 1 ? matches[1] | $ambiguous_match;
  endmethod

  method require_unique owner: ARCH_WIZARD
    "Return the sole candidate, with shared not-found and ambiguity diagnostics.";
    set_task_perms(caller_perms());
    const {text, matches, ?label = "object"} = args;
    matches || raise(E_INVARG, "No " + label + " found matching '" + text + "'.");
    if (length(matches) > 1)
      const shown = matches[1..min(8, length(matches))];
      const names = {tostr(object.name, " (", object, ")") for object in (shown)};
      raise(E_INVARG, "More than one match for '" + text + "'. Candidates: " + names:join(", ") + ". Use a more specific name or object number.");
    endif
    return matches[1];
  endmethod

  method environment owner: ARCH_WIZARD
    "Use the same visible scope as commands; non-player contexts expose their contents.";
    set_task_perms(caller_perms());
    const {context} = args;
    !valid(context) && return {};
    is_player(context) && return context:match_environment();
    return {context, @context.contents, @(valid(context.location) ? {context.location, @context.location.contents} | {})};
  endmethod

  method test_match_object owner: ARCH_WIZARD
    "Test object matching - returns actual objects.";
    let result = this:match_object("#1");
    result != #1 && raise(E_ASSERT, "Should return actual object #1: " + toliteral(result));
    result = this:match_object("$root");
    result != $root && raise(E_ASSERT, "Should return actual $root object: " + toliteral(result));
    try
      this:match_object(1);
      raise(E_ASSERT, "Non-string references must raise E_TYPE");
    except (E_TYPE)
    endtry
    try
      result = this:match_object("");
      raise(E_ASSERT, "Empty string should raise error, got: " + toliteral(result));
    except e (E_INVARG)
    endtry
    "Test environmental references";
    result = this:match_object("here");
    result == player.location || raise(E_ASSERT, "Should return player location for 'here': " + toliteral(result));
    result = this:match_object("me");
    result == player || raise(E_ASSERT, "Should return player for 'me': " + toliteral(result));
    result = this:match_object("player");
    result == player || raise(E_ASSERT, "Should return player for 'player': " + toliteral(result));
    "=== Named objects ===";
    const test_players = players();
    if (length(test_players) > 0)
      const first_player = test_players[1];
      const player_name = first_player.name;
      result = complex_match(player_name, test_players);
      result == first_player || raise(E_ASSERT, "Should return player object: " + toliteral(result));
    endif
    result = this:match_object(player.name);
    result == player || raise(E_ASSERT, "Should match the task player by name: " + toliteral(result));
  endmethod

  method test_resolve_in_scope_literals owner: HACKER
    scope = {#49, #50};
    result = this:resolve_in_scope("#49", scope);
    result != #49 && raise(E_ASSERT, "Literal numeric ID should resolve to #49: " + toliteral(result));
    temp = create($root);
    try
      uuid_str = tostr(temp);
      result = this:resolve_in_scope(uuid_str, scope);
      result == $failed_match || raise(E_ASSERT, "Literal ID must stay in scope");
      this:resolve_in_scope(uuid_str, {temp}) == temp || raise(E_ASSERT, "Scoped UUID should resolve");
    finally
      temp:destroy();
    endtry
    result = this:resolve_in_scope("#999999", scope);
    result != #-3 && raise(E_ASSERT, "Unknown literal should fail: " + toliteral(result));
  endmethod

  method test_resolve_in_scope_aliases owner: HACKER
    scope = {{#49, "first room", "lobby"}, {#50, "first area"}};
    result = this:resolve_in_scope("lobby", scope);
    result != #49 && raise(E_ASSERT, "Alias should resolve to first room: " + toliteral(result));
    result = this:resolve_in_scope("first area", scope);
    result != #50 && raise(E_ASSERT, "Text alias should resolve to first area: " + toliteral(result));
  endmethod

  method test_resolve_in_scope_ordinals owner: HACKER
    scope = {{#49, "room"}, {#50, "room"}};
    result = this:resolve_in_scope("second room", scope);
    result != #50 && raise(E_ASSERT, "Ordinal should pick second entry: " + toliteral(result));
    result = this:resolve_in_scope("third room", scope);
    result != #-3 && raise(E_ASSERT, "Out-of-range ordinal should fail: " + toliteral(result));
  endmethod

  method test_resolve_in_scope_fuzzy owner: HACKER
    scope = {{#49, "lobby"}};
    result = this:resolve_in_scope("lobbi", scope, ['fuzzy_threshold -> 0.8]);
    result != #49 && raise(E_ASSERT, "Fuzzy match should succeed with threshold: " + toliteral(result));
    result = this:resolve_in_scope("lobbi", scope, ['fuzzy_threshold -> 0.0]);
    result != #-3 && raise(E_ASSERT, "Fuzzy disabled should fail: " + toliteral(result));
  endmethod

  method object_suggestions owner: ARCH_WIZARD
    "Build labelled choices using the same names and scope order as object matching.";
    set_task_perms(caller_perms());
    const {objects, keys} = this:normalize_scope(args[1]);
    let result = {};
    for i in [1..length(objects)]
      const object = objects[i];
      result = {@result, ["id" -> tostr(object), "label" -> object:name(), "value" -> tostr(object),
        "detail" -> tostr(object), "objectKind" -> object:reference_kind(), "keys" -> keys[i]]};
    endfor
    return result;
  endmethod

  method input_context owner: HACKER
    "Describe the active template slot and any already-bound arguments.";
    const {template, ?active = "input", ?bindings = []} = args;
    typeof(template) == TYPE_STR && length(template) <= 1024 && typeof(bindings) == TYPE_MAP || raise(E_INVARG);
    active in {"input", "dobj", "iobj"} || raise(E_INVARG);
    let slots = {};
    for name in ({"input", "dobj", "iobj"})
      const marker = "{" + name + "}";
      if (index(template, marker))
        index(template, marker) == rindex(template, marker) || raise(E_INVARG, "Repeated argument slot");
        slots = {@slots, name};
      endif
    endfor
    active in slots && length(slots) <= 2 || raise(E_INVARG, "Missing active argument slot");
    for name in (mapkeys(bindings))
      name in slots || raise(E_INVARG, "Unknown bound argument");
      const value = bindings[name];
      typeof(value) == TYPE_STR && length(value) <= 256 && !index(value, "\n") && !index(value, "\r") || raise(E_INVARG);
    endfor
    return ["template" -> template, "active" -> active, "bindings" -> bindings, "slots" -> slots];
  endmethod

  method normalize_input_context owner: HACKER
    "Validate incoming context once, deriving unresolved slots from the template.";
    const {context} = args;
    typeof(context) == TYPE_MAP || raise(E_TYPE);
    !length(context) && return [];
    return this:input_context(context["template"], context["active"], `context["bindings"] ! E_RANGE => []');
  endmethod

  method matching_suggestions owner: ARCH_WIZARD
    "Use the ordinary command matcher, with provisional signature checks for an unfilled second slot.";
    set_task_perms(caller_perms());
    const {candidates, context, match_env, command_env} = args;
    !length(context) && return candidates;
    let template = context["template"];
    for slot in (context["slots"])
      const value = slot == context["active"] ? "#-2" | `context["bindings"][slot] ! E_RANGE => "#-3"';
      template = strsub(template, "{" + slot + "}", value);
    endfor
    const parsed = parse_command(template, match_env, true, 0.3);
    const direct = parsed['dobjstr] == "#-2";
    direct || parsed['iobjstr] == "#-2" || raise(E_INVARG, "Input must fill an object argument");
    const active_key = direct ? 'dobj | 'iobj;
    const other_key = direct ? 'iobj | 'dobj;
    const other_string = direct ? 'iobjstr | 'dobjstr;
    const provisional = parsed[other_string] == "#-3";
    if (provisional)
      "Probe possible receivers once, with a nonempty active sentinel. This accepts only an 'any' active argspec.";
      "Receiver probes do not enumerate combinations of candidate arguments.";
      const receivers = this:object_suggestions({@command_env, @match_env});
      for receiver in (receivers)
        let probe = parsed;
        probe[active_key] = #-2;
        probe[other_key] = toobj(receiver["value"]);
        find_command_verb(probe, command_env) && return candidates;
      endfor
    endif
    const other_candidates = parsed[other_key] == $ambiguous_match ? parsed[direct ? 'ambiguous_iobj | 'ambiguous_dobj] | {parsed[other_key]};
    let result = {};
    for candidate in (candidates)
      const object = toobj(candidate["value"]);
      let command = parsed;
      command[active_key] = object;
      command[direct ? 'dobjstr | 'iobjstr] = candidate["value"];
      "An unfilled slot can be any object, or the same receiver for a 'this' constraint.";
      const others = provisional ? {#-3, object} | other_candidates;
      for other in (others)
        command[other_key] = other;
        if (find_command_verb(command, command_env))
          result = {@result, candidate};
          break;
        endif
      endfor
    endfor
    return result;
  endmethod

  method rank_suggestions owner: HACKER
    "Rank exact, prefix, word-prefix, then substring matches; bound returned rows and omit search keys.";
    const {candidates, query, limit} = args;
    const needle = query:trim():lowercase();
    let buckets = {{}, {}, {}, {}};
    let seen = [];
    let count = 0;
    for candidate in (candidates)
      const id = candidate["id"];
      if (maphaskey(seen, id))
        continue;
      endif
      const keys = {candidate["label"], candidate["value"], @`candidate["keys"] ! E_RANGE => {}'};
      let rank = 5;
      for key in (keys)
        if (typeof(key) != TYPE_STR)
          continue;
        endif
        const text = key:lowercase();
        if (!needle || text == needle)
          rank = 1;
          break;
        elseif (index(text, needle) == 1)
          rank = min(rank, 2);
        elseif (index(text, " " + needle))
          rank = min(rank, 3);
        elseif (index(text, needle))
          rank = min(rank, 4);
        endif
      endfor
      if (rank == 5)
        continue;
      endif
      seen[id] = true;
      count = count + 1;
      if (length(buckets[rank]) < limit)
        buckets[rank] = {@buckets[rank], ["id" -> id, "label" -> candidate["label"], "value" -> candidate["value"], "detail" -> `candidate["detail"] ! E_RANGE => ""', "objectKind" -> `candidate["objectKind"] ! E_RANGE => "object"']};
      endif
    endfor
    const ranked = {@buckets[1], @buckets[2], @buckets[3], @buckets[4]};
    return ["items" -> ranked[1..min(length(ranked), limit)], "more" -> count > limit];
  endmethod
endobject
