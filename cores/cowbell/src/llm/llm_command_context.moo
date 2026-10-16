object LLM_COMMAND_CONTEXT [
  import_export_id -> "llm_command_context",
  import_export_hierarchy -> {"llm"}
]
  name: "LLM Command Context"
  parent: ROOT
  owner: ARCH_WIZARD

  method command_specs owner: ARCH_WIZARD
    "Read bounded public command signatures through native permissions, without calling target metadata methods.";
    const {actor, target} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const local = verbs(target);
    const chain = {target, @ancestors(target)};
    length(chain) <= 64 || raise(E_QUOTA, "Command ancestry is too deep.");
    let result = {};
    for definer in (chain)
      const names = definer == target ? local | `verbs(definer) ! E_PERM => {}';
      length(names) <= 512 || raise(E_QUOTA, "Too many command definitions.");
      for name in (names)
        const info = `verb_info(definer, name) ! E_PERM => 0';
        if (typeof(info) != TYPE_LIST || !index(info[2], "r"))
          continue;
        endif
        const signature = verb_args(definer, name);
        if (signature == {"this", "none", "this"})
          continue;
        endif
        const record = {info[3], definer, @signature};
        if (!(record in result))
          result = {@result, record};
          length(result) <= 128 && value_bytes(result) <= 32768 || raise(E_QUOTA, "Command catalog is too large.");
        endif
      endfor
    endfor
    return result;
  endmethod

  method signatures owner: ARCH_WIZARD
    "Render command signatures as inert syntax, preserving aliases and native argument specifications.";
    const {actor, target, ambient} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    let result = {};
    for spec in ($llm_command_context:command_specs(actor, target))
      const {names, definer, direct, prep, indirect} = spec;
      if (ambient && (direct == "this" || indirect == "this"))
        continue;
      endif
      if (!ambient && !(direct == "this" || indirect == "this" || direct == "any" || indirect == "any"))
        continue;
      endif
      let syntax = strsub(names, "*", "");
      direct != "none" && (syntax = syntax + (direct == "this" ? " <this>" | " <object>"));
      prep != "none" && (syntax = syntax + " " + prep);
      indirect != "none" && (syntax = syntax + (indirect == "this" ? " <this>" | " <object>"));
      result = setadd(result, syntax);
    endfor
    return result;
  endmethod

  method names owner: ARCH_WIZARD
    "Read bounded display names under the requester, including names used in occupancy context.";
    const {actor, objects} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    typeof(objects) == TYPE_LIST && length(objects) <= 256 || raise(E_QUOTA, "Too many named objects.");
    let result = {};
    for target in (objects)
      if (typeof(target) == TYPE_OBJ && valid(target))
        const name = target:name();
        typeof(name) == TYPE_STR && length(name) <= 256 || raise(E_INVARG, "Invalid object name.");
        result = {@result, name};
      endif
    endfor
    return result;
  endmethod

  method object_info owner: ARCH_WIZARD
    "Collect names, readable commands, and visible object state as the actor; private state stays absent.";
    const {actor, target} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const name = $llm_command_context:names(actor, {target})[1];
    const aliases = target:aliases();
    typeof(aliases) == TYPE_LIST && length(aliases) <= 16 || raise(E_QUOTA, "Too many object aliases.");
    for alias in (aliases)
      typeof(alias) == TYPE_STR && length(alias) <= 256 || raise(E_INVARG, "Invalid object alias.");
    endfor
    const signatures = `$llm_command_context:signatures(actor, target, false) ! E_PERM => {}';
    let info = ["name" -> name, "aliases" -> aliases, "commands" -> signatures[1..min(32, length(signatures))]];
    if (isa(target, $container))
      try
        let state = "closed";
        if (target.open)
          state = "open";
        elseif (target.locked)
          state = "closed and locked";
        endif
        info["state"] = state;
      except (E_PERM)
        "Private container state is not model context.";
      endtry
    endif
    for field in ({"sitting", "swimmers"})
      try
        const occupants = target.(field);
        if (typeof(occupants) == TYPE_LIST && length(occupants) > 0)
          info[field == "sitting" ? "occupied_by" | "swimmers"] = $llm_command_context:names(actor, occupants);
        endif
      except (E_PERM, E_PROPNF)
        "Missing or private occupancy is not model context.";
      endtry
    endfor
    value_bytes(info) <= 8192 || raise(E_QUOTA, "Object command context is too large.");
    return info;
  endmethod

  method objects owner: ARCH_WIZARD
    "Collect at most 30 visible objects with bounded metadata, preserving their order.";
    const {actor, objects} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    length(objects) <= 256 || raise(E_QUOTA, "Too many visible objects.");
    let result = {};
    for target in (objects)
      if (valid(target))
        result = {@result, $llm_command_context:object_info(actor, target)};
      endif
      if (length(result) == 30)
        break;
      endif
    endfor
    value_bytes(result) <= 32768 || raise(E_QUOTA, "Visible command context is too large.");
    return result;
  endmethod

  method room_commands owner: ARCH_WIZARD
    "Collect maintained exit commands and exact authored room hints without executing them.";
    const {actor, room} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const options = $room:exit_options(room, actor);
    length(options) <= 64 || raise(E_QUOTA, "Too many exits for command assistance.");
    let commands = {};
    let exits = {};
    let bindings = {};
    for option in (options)
      let names = {};
      if (maphaskey(option, 'commands))
        names = option['commands];
        bindings = {@bindings, {option['provider], option['identity], option['descriptor]}};
      elseif (maphaskey(option, 'passage))
        const aliases = $passage:expand_direction_aliases($passage:command_names(option['passage], room));
        names = {@aliases, @{"go " + alias for alias in (aliases)}};
        bindings = {@bindings, {option['identity], option['destination]}};
      endif
      exits = {@exits, ["label" -> option['label], "state" -> tostr(option['state]), "commands" -> names]};
      if (option['state] != 'unavailable)
        for command in (names)
          commands = setadd(commands, command);
        endfor
      endif
    endfor
    const hints = room:command_hints();
    typeof(hints) == TYPE_LIST && length(hints) <= 32 || raise(E_QUOTA, "Too many room command hints.");
    let safe_hints = {};
    for hint in (hints)
      const command = hint["command"];
      const description = `hint["description"] ! E_RANGE => ""';
      typeof(command) == TYPE_STR && length(command) > 0 && length(command) <= 512
        && typeof(description) == TYPE_STR && length(description) <= 512 || raise(E_INVARG, "Invalid room command hint.");
      safe_hints = {@safe_hints, ["command" -> command, "description" -> description]};
      commands = setadd(commands, command);
    endfor
    const result = ['commands -> commands, 'exits -> exits, 'bindings -> bindings, 'hints -> safe_hints];
    value_bytes(result) <= 32768 || raise(E_QUOTA, "Room command context is too large.");
    return result;
  endmethod

  method snapshot owner: ARCH_WIZARD
    "Capture parser scope and bounded inert context for one LLM command suggestion.";
    const {actor} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    const room = actor.location;
    valid(room) || raise(E_INVARG, "Actor is not in a room.");
    const scope = actor:match_environment("", ['complex -> true]);
    const environment = actor:command_environment();
    length(scope) <= 256 && length(environment) <= 128 || raise(E_QUOTA, "Command scope is too large.");
    let visible = {};
    for entry in (scope)
      const target = typeof(entry) == TYPE_LIST ? entry[1] | entry;
      if (typeof(target) == TYPE_OBJ && valid(target) && target != actor && target != room && !(target in actor.contents))
        visible = setadd(visible, target);
      endif
    endfor
    let ambient = {};
    for target in (environment)
      for signature in (`$llm_command_context:signatures(actor, target, true) ! E_PERM => {}')
        ambient = setadd(ambient, signature);
      endfor
    endfor
    length(ambient) <= 256 || raise(E_QUOTA, "Too many ambient commands.");
    const travel = $llm_command_context:room_commands(actor, room);
    const people = sort(players());
    const online = connected_players();
    const known = people[1..min(256, length(people))];
    const names = $llm_command_context:names(actor, known);
    const players = {["name" -> names[index], "connected" -> (known[index] in online) > 0] for index in [1..length(known)]};
    const data = ["location" -> $llm_command_context:names(actor, {room})[1],
      "inventory" -> $llm_command_context:objects(actor, actor.contents),
      "nearby" -> $llm_command_context:objects(actor, visible), "ambient_commands" -> ambient,
      "players" -> players, "exits" -> travel['exits], "room_hints" -> travel['hints]];
    actor.location == room || raise(E_PERM, "Actor moved during command context collection.");
    return ['data -> data, 'room -> room, 'scope -> scope, 'environment -> environment,
      'known_players -> known, 'commands -> travel['commands], 'exit_bindings -> travel['bindings]];
  endmethod

  method accepts owner: ARCH_WIZARD
    "Check candidate syntax against the native dispatcher or exact current room commands, without execution.";
    const {actor, snapshot, candidate} = args;
    $llm_tool_provider:_require_tool_dispatch(actor, caller_perms());
    set_task_perms(actor);
    actor.location == snapshot['room] || raise(E_PERM, "Actor moved.");
    typeof(candidate) == TYPE_STR && length(candidate) > 0 && length(candidate) <= 512
      && !index(candidate, "\n") && !index(candidate, "\r") || return false;
    candidate in snapshot['commands] && return true;
    const parsed = $match:parse_command(candidate, snapshot['scope], actor);
    const matches = find_command_verb(parsed, snapshot['environment]);
    return length(matches) > 0;
  endmethod
endobject
