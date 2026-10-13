object REACTION [
  import_export_id -> "reaction",
  import_export_hierarchy -> {"rules"}
]
  name: "Reaction Prototype"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property comparison_ops (owner: HACKER, flags: "rc") = {'eq, 'ne, 'gt, 'lt, 'ge, 'le};
  property effect_types (owner: HACKER, flags: "rc") = {
    'set,
    'increment,
    'decrement,
    'announce,
    'emote,
    'tell,
    'move,
    'trigger,
    'delay,
    'action
  };
  property event_triggers (owner: HACKER, flags: "rc") = {
    'on_get,
    'on_drop,
    'on_take,
    'on_put,
    'on_open,
    'on_close,
    'on_lock,
    'on_unlock,
    'on_enter,
    'on_leave,
    'on_pet,
    'on_wear,
    'on_remove,
    'on_use,
    'on_sit,
    'on_stand,
    'on_sittable_squeeze,
    'on_sittable_dump
  };

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for reactive behaviors on objects.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# $reaction - Reactive Behavior System",
    "",
    "## Overview",
    "",
    "The Reaction System provides a declarative way to define dynamic behaviors on objects in response",
    "to various events. Instead of writing complex imperative verb code, builders can specify high-level",
    "triggers, conditions, and effects. This system is designed to simplify common interaction patterns,",
    "access controls, and environmental responses.",
    "",
    "Reactions are essentially encapsulated rules that execute specific actions when their criteria are met.",
    "They are particularly useful for creating:",
    "- **Interactive objects**: Doors that unlock, containers that react to items.",
    "- **Environmental responses**: Objects changing state based on player actions.",
    "- **Simple AI behaviors**: NPCs reacting to stimuli.",
    "",
    "## How It Works",
    "",
    "Each reaction is composed of three main parts:",
    "1.  **Trigger**: The event that initiates the reaction check. This can be a simple symbolic event (e.g., `'on_unlock`, `'on_pet`) or a more complex threshold condition.",
    "2.  **When Clause (Condition)**: An optional logical expression, evaluated by the `$rule_engine`, that must be true for the reaction's effects to fire. This allows for conditional behaviors (e.g., 'only if locked', 'only if owner').",
    "3.  **Effects**: A list of actions to perform if the trigger fires and the 'when' clause is satisfied. These effects range from setting properties to sending messages or triggering other events.",
    "",
    "## Creating Reactions",
    "",
    "Use `$reaction:mk` to construct a reaction. Use `$reaction:install(host, property, reaction)` to authorize it on a named `_reaction` property.",
    "",
    "The caller must own the host or be a wizard. Installation binds the exact definition to the host, property, and current owner.",
    "Direct property edits and ownership transfers invalidate that authorization. Reinstall the reaction after an approved change.",
    "Builder commands install and authorize reactions for the builder. Direct `fire_trigger` requests require the host owner or a wizard.",
    "Delayed effects recheck installation before execution. State effects cannot write built-in identity or security properties.",
    "",
    "### Basic Event Trigger",
    "",
    "A reaction that fires on a simple event, with no special conditions:",
    "```moo",
    "reaction = $reaction:mk('on_unlock, 0, {",
    "  {'announce, \"{nc} hears a click from {i}.\"},",
    "  {'set, 'locked, false}",
    "});",
    "```",
    "",
    "Here:",
    "- `'on_unlock`: The symbolic event that triggers this reaction.",
    "- `0`: No 'when' clause (always fires if triggered).",
    "- `{{'announce, ...}, {'set, ...}}`: A list of effects to execute.",
    "",
    "### Reaction with a Condition (`when` clause)",
    "",
    "Conditions use the `$rule_engine` syntax to define prerequisites for effects to run:",
    "```moo",
    "reaction = $reaction:mk('on_pet, \"NOT property_is(object: input.This, field: 'mood, value: \\\"grouchy\\\")\", {",
    "  {'increment, 'pets_received},",
    "  {'emote, \"purrs contentedly.\"}",
    "});",
    "```",
    "",
    "This reaction will only fire if the `on_pet` event occurs *and* the object (`This`) does not have a grouchy mood.",
    "",
    "### Threshold Triggers",
    "",
    "Threshold triggers allow reactions to fire when an object's property crosses a specific value, rather than on a discrete event. The trigger is specified as a list:",
    "```moo",
    "reaction = $reaction:mk({'when, 'pets_received, 'ge, 10}, 0, {",
    "  {'set, 'mood, \"happy\"},",
    "  {'emote, \"seems to have warmed up.\"}",
    "});",
    "```",
    "",
    "This reaction fires when `pets_received` becomes greater than or equal to `10`. The possible comparison operators are (`'eq`, `'ne`, `'gt`, `'lt`, `'ge`, `'le`).",
    "",
    "## Effect Types",
    "",
    "The reaction system supports a variety of built-in effects. Messages within `announce`, `emote`, and `tell` effects can use `$sub_utils` templating.",
    "",
    "| Effect Type   | Syntax                               | Description                                                                  |",
    "|---------------|--------------------------------------|------------------------------------------------------------------------------|",
    "| `'set`         | `{'set, 'prop_name, value}`          | Sets a property `prop_name` on the reacting object to `value`.               |",
    "| `'increment`   | `{'increment, 'prop_name, ?amount}` | Adds `amount` (default 1) to a numeric property `prop_name`.                 |",
    "| `'decrement`   | `{'decrement, 'prop_name, ?amount}` | Subtracts `amount` (default 1) from a numeric property `prop_name`.          |",
    "| `'announce`    | `{'announce, \"message template\"}`   | Sends a message to the reacting object's location, parsed by `$sub_utils`.   |",
    "| `'emote`       | `{'emote, \"message template\"}`      | Causes the reacting object to emote in its location, parsed by `$sub_utils`. |",
    "| `'tell`        | `{'tell, 'RecipientVar, \"message\"}` | Sends a private message to a recipient (resolved from context), parsed by `$sub_utils`. |",
    "| `'move`        | `{'move, destination}`                | Moves the reacting object to the `destination` object.                       |",
    "| `'trigger`     | `{'trigger, target_obj, 'event_sym}` | Fires another symbolic event on `target_obj`.                                |",
    "| `'delay`       | `{'delay, seconds, inner_effect}`     | Schedules `inner_effect` to execute after `seconds` in a forked task.      |",
    "| `'action`      | `{'action, 'verb_name, target}`       | Calls `target:action_<verb_name>(this, context)` for extensible behaviors.   |",
    "",
    "## Action Effects",
    "",
    "The `'action` effect provides an extensible way to invoke behaviors on other objects. When executed:",
    "",
    "1. The `target` is resolved (if a symbol, looked up in context; otherwise used directly)",
    "2. The verb `action_<verb_name>` is called on the target with `(reacting_object, context)`",
    "",
    "This allows objects to define their own action handlers. For example, `$sittable` defines:",
    "",
    "- `action_sit(who, context)` - makes `who` sit on the furniture",
    "- `action_stand(who, context)` - makes `who` stand up from the furniture",
    "",
    "Example reaction making Henri sit on a couch when the cupboard opens:",
    "",
    "```moo",
    "$reaction:install(henri, \"sit_reaction\", $reaction:mk('on_cupboard_open, 0, {{'action, 'sit, some_couch}}));",
    "```",
    "",
    "Or with a context variable:",
    "",
    "```moo",
    "$reaction:install(henri, \"sit_reaction\", $reaction:mk('on_cupboard_open, 0, {{'action, 'sit, 'Furniture}}));",
    "// Fire with: henri:fire_trigger('on_cupboard_open, ['Furniture -> some_couch])",
    "```",
    "",
    "## Message Templates",
    "",
    "As noted above, `announce`, `emote`, and `tell` effects leverage the `$sub_utils` template language for dynamic message generation. This allows messages to include information about the actor, direct/indirect objects, pronouns, and even self-alternating phrases.",
    "",
    "Examples of `$sub_utils` tokens:",
    "- `{nc}`: Capitalized name of the actor.",
    "- `{d}`: Name of the direct object.",
    "- `{i}`: Name of the indirect object (often the reacting object itself).",
    "- `{s}`: Subject pronoun for the actor (e.g., \"he\", \"she\", \"they\").",
    "- `{feel|feels}`: A self-alternating phrase that displays differently to the actor versus others.",
    "",
    "For full details on template syntax, see `$sub_utils` documentation.",
    "",
    "## Adding Reactions to Objects",
    "",
    "Reactions are stored as properties ending with `_reaction`. Use `@add-reaction` or install them from MOO code:",
    "",
    "```moo",
    "$reaction:install(chest, \"unlock_reaction\", $reaction:mk('on_unlock, 0, {{'announce, \"Click!\"}}));",
    "$reaction:install(chest, \"open_reaction\", $reaction:mk('on_open, 0, {{'emote, \"creaks open.\"}}));",
    "```",
    "",
    "Installed reactions run while enabled and their authorization remains valid. Use `@enable-reaction` and `@disable-reaction` to toggle them.",
    "",
    "## Message Property References",
    "",
    "Instead of inline message strings, effects can reference message properties by symbol:",
    "",
    "```moo",
    "cat.pet_msg = $sub_utils:compile(\"purrs contentedly.\");",
    "$reaction:install(cat, \"pet_reaction\", $reaction:mk('on_pet, 0, {{'emote, 'pet_msg}}));",
    "```",
    "",
    "If the property is a `$msg_bag`, a random message is picked each time. This allows varying responses:",
    "",
    "```moo",
    "cat.pet_msgs = $msg_bag:create(true);",
    "cat.pet_msgs:add($sub_utils:compile(\"purrs contentedly.\"));",
    "cat.pet_msgs:add($sub_utils:compile(\"rubs against {p} legs.\"));",
    "$reaction:install(cat, \"pet_reaction\", $reaction:mk('on_pet, 0, {{'emote, 'pet_msgs}}));",
    "```"
  };

  method mk owner: HACKER
    "Create an enabled reaction flyweight from a trigger, optional when clause, and effect specs.";
    "Args: trigger, when_clause (0 or rule string), effects (list of effect specs)";
    "Returns: <REACTION, .trigger, .when, .effects, .enabled, .fired_at>.";
    const {trigger, ?when_clause = 0, ?effects = {}} = args;
    "Validate trigger";
    this:validate_trigger(trigger);
    "Parse when clause if provided";
    let parsed_when = 0;
    if (typeof(when_clause) == TYPE_STR && length(when_clause) > 0)
      parsed_when = $rule_engine:parse_expression(when_clause, 'reaction_condition, $rule_engine:trigger_scope(trigger));
    elseif (typeof(when_clause) != TYPE_INT || when_clause != 0)
      raise(E_INVARG, "Reaction condition must be zero or a nonempty rule string");
    endif
    "Parse and validate effects";
    let parsed_effects = {};
    for effect_spec in (effects)
      parsed_effects = {@parsed_effects, this:parse_effect(effect_spec)};
    endfor
    return <this, .trigger = trigger, .when = parsed_when, .effects = parsed_effects, .enabled = true, .fired_at = 0>;
  endmethod

  method validate_trigger owner: HACKER
    "Validate an event-symbol trigger or threshold trigger list.";
    "Threshold form is {'when, property_symbol, comparison_op, threshold_value}.";
    {trigger} = args;
    "Event trigger - just a symbol";
    if (typeof(trigger) == TYPE_SYM)
      return true;
    endif
    "Threshold trigger - {'when, 'prop, 'op, value}";
    if (typeof(trigger) == TYPE_LIST)
      length(trigger) == 4 || raise(E_INVARG, "Threshold trigger must be {'when, 'prop, 'op, value}");
      {kind, prop, op, value} = trigger;
      kind == 'when || raise(E_INVARG, "Threshold trigger must start with 'when");
      typeof(prop) == TYPE_SYM || raise(E_INVARG, "Property must be symbol");
      op in this.comparison_ops || raise(E_INVARG, "Invalid comparison operator: " + tostr(op));
      return true;
    endif
    raise(E_INVARG, "Trigger must be symbol or {'when, 'prop, 'op, value}");
  endmethod

  method parse_effect owner: HACKER
    "Parse one effect spec into a validated reaction-effect flyweight.";
    "String message effects are compiled through $sub_utils; list messages are preserved.";
    {spec} = args;
    typeof(spec) == TYPE_LIST || raise(E_INVARG, "Effect must be list");
    length(spec) >= 2 || raise(E_INVARG, "Effect must have at least type and one argument");
    effect_type = spec[1];
    effect_type in this.effect_types || raise(E_INVARG, "Unknown effect type: " + tostr(effect_type));
    if (effect_type == 'set)
      length(spec) == 3 || raise(E_INVARG, "'set requires {'set, 'prop, value}");
      {_, prop, value} = spec;
      return <$reaction, .type = 'set, .prop = prop, .value = value>;
    elseif (effect_type == 'increment)
      {_, prop, ?by = 1} = spec;
      return <$reaction, .type = 'increment, .prop = prop, .by = by>;
    elseif (effect_type == 'decrement)
      {_, prop, ?by = 1} = spec;
      return <$reaction, .type = 'decrement, .prop = prop, .by = by>;
    elseif (effect_type == 'announce)
      {_, msg} = spec;
      compiled_msg = typeof(msg) == TYPE_STR ? $sub_utils:compile(msg) | msg;
      return <$reaction, .type = 'announce, .msg = compiled_msg>;
    elseif (effect_type == 'emote)
      {_, msg} = spec;
      compiled_msg = typeof(msg) == TYPE_STR ? $sub_utils:compile(msg) | msg;
      return <$reaction, .type = 'emote, .msg = compiled_msg>;
    elseif (effect_type == 'tell)
      {_, target_var, msg} = spec;
      compiled_msg = typeof(msg) == TYPE_STR ? $sub_utils:compile(msg) | msg;
      return <$reaction, .type = 'tell, .target = target_var, .msg = compiled_msg>;
    elseif (effect_type == 'move)
      {_, dest} = spec;
      return <$reaction, .type = 'move, .destination = dest>;
    elseif (effect_type == 'trigger)
      {_, target_obj, event} = spec;
      return <$reaction, .type = 'trigger, .target = target_obj, .event = event>;
    elseif (effect_type == 'delay)
      {_, seconds, inner_effect} = spec;
      parsed_inner = this:parse_effect(inner_effect);
      return <$reaction, .type = 'delay, .seconds = seconds, .effect = parsed_inner>;
    elseif (effect_type == 'action)
      {_, action_name, ?action_target = 0} = spec;
      typeof(action_name) == TYPE_SYM || raise(E_INVARG, "'action requires symbol for action name");
      return <$reaction, .type = 'action, .action = action_name, .action_target = action_target>;
    endif
    raise(E_INVARG, "Unhandled effect type: " + tostr(effect_type));
  endmethod

  method _definition owner: HACKER
    "Copy only the executable fields of a canonical reaction.";
    const {value} = args;
    typeof(value) == TYPE_FLYWEIGHT && value.delegate == $reaction || raise(E_INVARG, "Expected a reaction");
    $reaction:validate_trigger(value.trigger);
    (typeof(value.when) == TYPE_INT && value.when == 0) || (typeof(value.when) == TYPE_FLYWEIGHT && value.when.delegate == $rule) || raise(E_INVARG, "Invalid reaction condition");
    typeof(value.effects) == TYPE_LIST || raise(E_INVARG, "Reaction effects must be a list");
    return <$reaction, .trigger = value.trigger, .when = value.when, .effects = value.effects, .enabled = value.enabled, .fired_at = 0>;
  endmethod

  method install owner: ARCH_WIZARD
    "Authorize this exact reaction on one host property under its current owner.";
    const {host, property, value} = args;
    const principal = caller_perms();
    typeof(host) == TYPE_OBJ && valid(host) || raise(E_INVARG);
    principal == host.owner || principal.wizard || raise(E_PERM);
    const name = tostr(property);
    name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
    let reaction = $reaction:_definition(value);
    "CBOR preserves anonymous references and value types inside the signed snapshot.";
    const snapshot = {host, name, host.owner, host.owner.wizard, reaction};
    const claims = ['kind -> "cowbell.reaction", 'installation -> encode_base64(encode_cbor(snapshot))];
    reaction.authorization = paseto_make_local(claims);
    set_task_perms(principal);
    if ($obj_utils:property_key(host, name) == E_PROPNF)
      add_property(host, name, reaction, {principal, "r"});
    else
      host.(name) = reaction;
    endif
    return reaction;
  endmethod

  method _authority owner: ARCH_WIZARD
    "Verify the installed value and owner before returning an execution principal.";
    const {host, property, reaction} = args;
    typeof(host) == TYPE_OBJ && valid(host) || return 0;
    reaction.enabled || return 0;
    const token = `reaction.authorization ! E_PROPNF => 0';
    if (token != 0)
      let claims;
      try
        claims = paseto_verify_local(token);
        claims["kind"] == "cowbell.reaction" || return 0;
        const {approved_host, name, owner, wizard, definition} = decode_cbor(decode_base64(claims["installation"]));
        approved_host == host && name == tostr(property) || return 0;
        owner == host.owner && wizard == owner.wizard || return 0;
        equal(encode_cbor(host.(name)), encode_cbor(reaction)) || return 0;
        equal(encode_cbor(definition), encode_cbor($reaction:_definition(reaction))) || return 0;
        return owner;
      except (E_INVARG, E_TYPE, E_RANGE, E_PROPNF)
        return 0;
      endtry
    endif
    return 0;
  endmethod

  method change owner: ARCH_WIZARD
    "Mutate an owned state property and dispatch thresholds from the actual before/after values.";
    const {host, property, operation, value, ?context = []} = args;
    const principal = caller_perms();
    typeof(host) == TYPE_OBJ && valid(host) || raise(E_INVARG);
    principal == host.owner || principal.wizard || raise(E_PERM);
    operation in {'set, 'increment, 'decrement} || raise(E_INVARG, "Unknown state operation");
    typeof(property) == TYPE_SYM || typeof(property) == TYPE_STR || raise(E_INVARG);
    const name = tostr(property);
    "Built-in identity and security properties are not behavioral state.";
    name in {"name", "owner", "location", "contents", "programmer", "wizard", "r", "w", "f", "readable", "writable", "fertile"} && raise(E_PERM, "Reaction effects cannot change built-in object properties");
    typeof(context) == TYPE_MAP || raise(E_INVARG);
    set_task_perms(principal);
    const old_value = host.(name);
    const new_value = operation == 'set ? value | (operation == 'increment ? old_value + value | old_value - value);
    host.(name) = new_value;
    $reaction:_dispatch(host, 0, context, {tosym(name), old_value, new_value});
    return new_value;
  endmethod

  method _dispatch owner: ARCH_WIZARD
    "Dispatch only after the trigger entry or mutation method authenticates its caller.";
    const source = callers()[1];
    (source[4] == $root && source[2] == "fire_trigger") || (source[4] == $reaction && source[2] == "change") || raise(E_PERM);
    const {host, trigger_name, initial_context, ?transition = {}} = args;
    let context = initial_context;
    context['This] = host;
    context['Location] = host.location;
    for entry in ($root:_get_reactions_for(host, $arch_wizard, true))
      const {property, reaction} = entry;
      if (!reaction.enabled)
        continue;
      endif
      if (length(transition) == 0)
        if (reaction.trigger != trigger_name)
          continue;
        endif
      else
        const trigger = reaction.trigger;
        if (!(typeof(trigger) == TYPE_LIST && length(trigger) == 4 && trigger[1] == 'when))
          continue;
        endif
        const {prop, old_value, new_value} = transition;
        if (!(trigger[2] == prop && $reaction:threshold_crossed(old_value, new_value, trigger[3], trigger[4])))
          continue;
        endif
      endif
      $reaction:_run_installed(host, property, reaction, context);
    endfor
  endmethod

  method _run_installed owner: ARCH_WIZARD
    "Recheck installation before execution, including after a delayed continuation.";
    const source = callers()[1];
    source[4] == $reaction && source[2] in {"_dispatch", "execute_effect"} || raise(E_PERM);
    const {host, property, reaction, context, ?effect = 0} = args;
    const principal = $reaction:_authority(host, property, reaction);
    principal != 0 || return false;
    set_task_perms(principal);
    const installation = {host, property, reaction};
    if (effect == 0)
      return reaction:execute(context, installation);
    endif
    return $reaction:execute_effect(effect, context, host, installation);
  endmethod

  method compare owner: HACKER
    "Compare two values using one of the configured comparison operator symbols.";
    {left, op, right} = args;
    if (op == 'eq)
      return left == right;
    elseif (op == 'ne)
      return left != right;
    elseif (op == 'gt)
      return left > right;
    elseif (op == 'lt)
      return left < right;
    elseif (op == 'ge)
      return left >= right;
    elseif (op == 'le)
      return left <= right;
    endif
    return false;
  endmethod

  method threshold_crossed owner: HACKER
    "Return true when a value transition newly satisfies a threshold comparison.";
    {old_value, new_value, op, threshold} = args;
    was_met = this:compare(old_value, op, threshold);
    now_met = this:compare(new_value, op, threshold);
    return now_met && !was_met;
  endmethod

  method execute owner: ARCH_WIZARD
    "Execute this reaction in the given context if its when clause succeeds.";
    "Context is map: ['Actor -> player, 'This -> object, 'Key -> iobj, ...]";
    "Returns false when a when clause fails, otherwise true after all effects run.";
    set_task_perms(caller_perms());
    const {initial_context, ?installation = 0} = args;
    if (installation != 0)
      const source = callers()[1];
      source[4] == $reaction && source[2] == "_run_installed" || raise(E_PERM);
    endif
    let context = initial_context;
    "A zero when clause means the reaction is unconditional.";
    if (this.when != 0)
      const result = $rule_engine:evaluate(this.when, context);
      if (!result['success])
        return false;
      endif
      "Merge any new bindings into context";
      if (maphaskey(result, 'bindings))
        context = mapmerge(context, result['bindings]);
      endif
    endif
    "Execute each effect";
    const target = context['This];
    for effect in (this.effects)
      $reaction:execute_effect(effect, context, target, installation);
    endfor
    return true;
  endmethod

  method _resolve_msg owner: ARCH_WIZARD
    "Resolve a message reference to a compiled template list.";
    "If msg is a symbol, looks up that property on target.";
    "If the property is a $msg_bag (object or flyweight), picks randomly.";
    "Returns a list suitable for @-splat into $event:mk_info.";
    set_task_perms(caller_perms());
    const {msg, target} = args;
    "If it's a symbol, look up the property on target";
    if (typeof(msg) == TYPE_SYM)
      const prop_name = tostr(msg);
      const prop_value = `target.(prop_name) ! E_PROPNF => 0';
      prop_value == 0 && return {"(missing message: " + prop_name + ")"};
      "If it's a msg_bag (object or flyweight), pick randomly";
      if (typeof(prop_value) == TYPE_OBJ && isa(prop_value, $msg_bag))
        return prop_value:pick();
      elseif (typeof(prop_value) == TYPE_FLYWEIGHT && prop_value.delegate == $msg_bag)
        return prop_value:pick();
      endif
      "Otherwise use the property value directly (should be compiled list)";
      return prop_value;
    endif
    "Already a compiled list or other value";
    return msg;
  endmethod

  method execute_effect owner: ARCH_WIZARD
    "Execute a single parsed or raw effect against target using context bindings.";
    "Mutation effects also check threshold reactions on the same target.";
    set_task_perms(caller_perms());
    const {raw_effect, context, target, ?installation = 0} = args;
    if (installation != 0)
      const source = callers()[1];
      source[4] == $reaction && source[2] in {"execute", "_run_installed"} || raise(E_PERM);
      installation[1] == target || raise(E_PERM);
      $reaction:_authority(target, installation[2], installation[3]) == caller_perms() || return false;
    endif
    let effect = raw_effect;
    "Parse list effects on the fly for convenience";
    if (typeof(effect) == TYPE_LIST)
      effect = this:parse_effect(effect);
    endif
    const requested_actor = `context['Actor] ! E_RANGE => 0';
    const actor = typeof(requested_actor) == TYPE_OBJ && valid(requested_actor) ? requested_actor | player;
    if (effect.type in {'set, 'increment, 'decrement})
      const value = effect.type == 'set ? effect.value | effect.by;
      return $reaction:change(target, effect.prop, effect.type, value, context);
    elseif (effect.type == 'announce)
      "Announce to actor's room - actor is the player, dobj is the reacting object";
      const msg = this:_resolve_msg(effect.msg, target);
      const event = $event:mk_info(actor, @msg):with_dobj(target);
      const room = isa(actor, $player) ? actor.location | target.location;
      if (valid(room) && isa(room, $room))
        room:announce(event);
      endif
    elseif (effect.type == 'emote)
      "Object 'does' something - auto-prefix target name like emote command";
      const msg = this:_resolve_msg(effect.msg, target);
      const event = $event:mk_emote(target, target.name, " ", @msg):with_dobj(actor);
      const room = isa(actor, $player) ? actor.location | target.location;
      if (valid(room) && isa(room, $room))
        room:announce(event);
      endif
    elseif (effect.type == 'tell)
      "Private message to a specific target";
      const recipient = context[effect.target];
      if (valid(recipient))
        const msg = this:_resolve_msg(effect.msg, target);
        const event = $event:mk_info(actor, @msg):with_iobj(target);
        recipient:tell(event);
      endif
    elseif (effect.type == 'move)
      const dest = typeof(effect.destination) == TYPE_SYM ? context[effect.destination] | effect.destination;
      if (valid(dest))
        move(target, dest);
      endif
    elseif (effect.type == 'trigger)
      "Fire another trigger on another object";
      if (valid(effect.target))
        effect.target:fire_trigger(effect.event, context);
      endif
    elseif (effect.type == 'delay)
      "Fork commits earlier effects before this delayed effect runs.";
      fork (effect.seconds)
        if (installation == 0)
          $reaction:execute_effect(effect.effect, context, target);
        else
          $reaction:_run_installed(target, installation[2], installation[3], context, effect.effect);
        endif
      endfork
    elseif (effect.type == 'action)
      "Call action_<name> on action_target, passing the reacting object and context";
      let action_target = effect.action_target;
      if (typeof(action_target) == TYPE_SYM)
        action_target = context[action_target];
      endif
      if (valid(action_target))
        const verb_name = "action_" + tostr(effect.action);
        `action_target:(verb_name)(target, context) ! E_VERBNF';
      endif
    endif
  endmethod

  method test_mk_event_trigger owner: HACKER
    "Test creating a reaction with a simple event trigger.";
    reaction = this:mk('on_unlock, 0, {});
    $test_utils:assert_type(reaction, TYPE_FLYWEIGHT, "mk() should return a flyweight");
    $test_utils:assert_eq(reaction.delegate, this, "delegate should be $reaction");
    $test_utils:assert_eq(reaction.trigger, 'on_unlock, "trigger should be stored");
    $test_utils:assert_eq(reaction.when, 0, "default when clause should be disabled");
    $test_utils:assert_eq(reaction.effects, {}, "empty effects should be preserved");
    $test_utils:assert_true(reaction.enabled, "reactions should be enabled by default");
    return true;
  endmethod

  method test_mk_threshold_trigger owner: HACKER
    "Test creating a reaction with a threshold trigger.";
    reaction = this:mk({'when, 'pets_received, 'ge, 10}, 0, {});
    $test_utils:assert_type(reaction, TYPE_FLYWEIGHT, "mk() should return a flyweight");
    $test_utils:assert_type(reaction.trigger, TYPE_LIST, "threshold trigger should be a list");
    $test_utils:assert_eq(reaction.trigger[1], 'when, "threshold trigger kind");
    $test_utils:assert_eq(reaction.trigger[2], 'pets_received, "threshold trigger property");
    $test_utils:assert_eq(reaction.trigger[3], 'ge, "threshold trigger operator");
    $test_utils:assert_eq(reaction.trigger[4], 10, "threshold trigger value");
    return true;
  endmethod

  method test_mk_with_condition owner: HACKER
    "Test creating a reaction with a when condition.";
    reaction = this:mk('on_pet, "NOT property_is(object: input.This, field: 'mood, value: \"grouchy\")", {});
    $test_utils:assert_type(reaction, TYPE_FLYWEIGHT, "mk() should return a flyweight");
    $test_utils:assert_true(reaction.when != 0, "string when clause should be parsed");
    $test_utils:assert_type(reaction.when, TYPE_FLYWEIGHT, "when clause should be a rule flyweight");
    return true;
  endmethod

  method test_mk_invalid_trigger owner: HACKER
    "Test that invalid triggers raise errors.";
    $test_utils:assert_raises(E_INVARG, this, "mk", {"not_a_symbol", 0, {}}, "string triggers should be rejected");
    $test_utils:assert_raises(E_INVARG, this, "mk", {{'wrong, 'prop}, 0, {}}, "malformed threshold triggers should be rejected");
    return true;
  endmethod

  method test_parse_effect_set owner: HACKER
    "Test parsing a 'set' effect.";
    effect = this:parse_effect({'set, 'locked, false});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'set, "effect type");
    $test_utils:assert_eq(effect.prop, 'locked, "set property");
    $test_utils:assert_false(effect.value, "set value");
    return true;
  endmethod

  method test_parse_effect_increment owner: HACKER
    "Test parsing an 'increment' effect.";
    effect = this:parse_effect({'increment, 'counter});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'increment, "effect type");
    $test_utils:assert_eq(effect.prop, 'counter, "increment property");
    $test_utils:assert_eq(effect.by, 1, "default increment amount");
    effect2 = this:parse_effect({'increment, 'counter, 5});
    $test_utils:assert_eq(effect2.by, 5, "explicit increment amount");
    return true;
  endmethod

  method test_parse_effect_announce owner: HACKER
    "Test parsing an 'announce' effect with template compilation.";
    effect = this:parse_effect({'announce, "{nc} hears a click."});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'announce, "effect type");
    $test_utils:assert_type(effect.msg, TYPE_LIST, "string messages should compile to template lists");
    $test_utils:assert_true(length(effect.msg) > 0, "compiled message should have content");
    return true;
  endmethod

  method test_parse_effect_emote owner: HACKER
    "Test parsing an 'emote' effect.";
    effect = this:parse_effect({'emote, "purrs contentedly."});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'emote, "effect type");
    $test_utils:assert_type(effect.msg, TYPE_LIST, "string messages should compile to template lists");
    return true;
  endmethod

  method test_parse_effect_trigger owner: HACKER
    "Test parsing a 'trigger' effect.";
    effect = this:parse_effect({'trigger, $root, 'on_activate});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'trigger, "effect type");
    $test_utils:assert_eq(effect.target, $root, "trigger target");
    $test_utils:assert_eq(effect.event, 'on_activate, "trigger event");
    return true;
  endmethod

  method test_parse_effect_delay owner: HACKER
    "Test parsing a 'delay' effect with nested effect.";
    effect = this:parse_effect({'delay, 5, {'announce, "Boom!"}});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'delay, "effect type");
    $test_utils:assert_eq(effect.seconds, 5, "delay seconds");
    $test_utils:assert_type(effect.effect, TYPE_FLYWEIGHT, "nested effect should be parsed");
    $test_utils:assert_eq(effect.effect.type, 'announce, "nested effect type");
    return true;
  endmethod

  method test_parse_effect_invalid owner: HACKER
    "Test that invalid effects raise errors.";
    $test_utils:assert_raises(E_INVARG, this, "parse_effect", {{'unknown_type, "foo"}}, "unknown effect type should be rejected");
    $test_utils:assert_raises(E_INVARG, this, "parse_effect", {"not a list"}, "non-list effect should be rejected");
    return true;
  endmethod

  method test_mk_compiles_effects owner: HACKER
    "Test that mk() parses all effects.";
    reaction = this:mk('on_unlock, 0, {{'announce, "Click!"}, {'set, 'locked, false}, {'increment, 'times_unlocked}});
    $test_utils:assert_eq(length(reaction.effects), 3, "mk() should parse every effect");
    $test_utils:assert_eq(reaction.effects[1].type, 'announce, "first effect type");
    $test_utils:assert_eq(reaction.effects[2].type, 'set, "second effect type");
    $test_utils:assert_eq(reaction.effects[3].type, 'increment, "third effect type");
    return true;
  endmethod

  method test_compare owner: HACKER
    "Test comparison operators.";
    $test_utils:assert_true(this:compare(5, 'eq, 5), "eq should match equal values");
    $test_utils:assert_false(this:compare(5, 'eq, 3), "eq should reject unequal values");
    $test_utils:assert_true(this:compare(5, 'ne, 3), "ne should match unequal values");
    $test_utils:assert_true(this:compare(5, 'gt, 3), "gt should compare greater values");
    $test_utils:assert_false(this:compare(3, 'gt, 5), "gt should reject smaller values");
    $test_utils:assert_true(this:compare(3, 'lt, 5), "lt should compare smaller values");
    $test_utils:assert_true(this:compare(5, 'ge, 5), "ge should accept equality");
    $test_utils:assert_true(this:compare(5, 'ge, 3), "ge should accept greater values");
    $test_utils:assert_true(this:compare(5, 'le, 5), "le should accept equality");
    $test_utils:assert_true(this:compare(3, 'le, 5), "le should accept smaller values");
    return true;
  endmethod

  method test_threshold_crossed owner: HACKER
    "Test threshold crossing detection.";
    $test_utils:assert_true(this:threshold_crossed(9, 10, 'ge, 10), "9->10 crosses >=10");
    $test_utils:assert_true(this:threshold_crossed(5, 15, 'ge, 10), "5->15 crosses >=10");
    $test_utils:assert_false(this:threshold_crossed(10, 11, 'ge, 10), "10->11 already met >=10");
    $test_utils:assert_false(this:threshold_crossed(8, 9, 'ge, 10), "8->9 does not cross >=10");
    $test_utils:assert_true(this:threshold_crossed(9, 10, 'eq, 10), "9->10 crosses ==10");
    $test_utils:assert_false(this:threshold_crossed(10, 10, 'eq, 10), "10->10 already met ==10");
    $test_utils:assert_true(this:threshold_crossed(11, 10, 'eq, 10), "11->10 crosses ==10");
    return true;
  endmethod

  method test_execute_mutation_effects_and_thresholds owner: HACKER
    "Test mutation effects and threshold reactions during execution.";
    target = $test_utils:anonymous($root);
    add_property(target, "score", 1, {this.owner, "r"});
    add_property(target, "state", "idle", {this.owner, "r"});
    add_property(target, "threshold_hit", false, {this.owner, "r"});
    threshold = this:mk({'when, 'score, 'ge, 5}, 0, {{'set, 'threshold_hit, true}});
    $reaction:install(target, "score_threshold_reaction", threshold);
    reaction = this:mk('on_score, 0, {{'increment, 'score, 4}, {'decrement, 'score, 2}, {'set, 'state, "done"}});
    $test_utils:assert_true(reaction:execute(['Actor -> player, 'This -> target]), "mutation reaction should execute");
    $test_utils:assert_eq(target.score, 3, "increment and decrement effects should mutate numeric properties");
    $test_utils:assert_eq(target.state, "done", "set effect should update target property");
    $test_utils:assert_true(target.threshold_hit, "threshold reaction should fire when score crosses threshold");
    return true;
  endmethod

  method test_fire_trigger_enabled_filtering owner: HACKER
    "Test root fire_trigger dispatches matching enabled reactions only.";
    target = $test_utils:anonymous($root);
    add_property(target, "count", 0, {this.owner, "r"});
    enabled_reaction = this:mk('on_ping, 0, {{'increment, 'count, 1}});
    disabled_reaction = this:mk('on_ping, 0, {{'increment, 'count, 100}});
    disabled_reaction.enabled = false;
    $reaction:install(target, "enabled_ping_reaction", enabled_reaction);
    $reaction:install(target, "disabled_ping_reaction", disabled_reaction);
    target:fire_trigger('on_ping, ['Actor -> player]);
    $test_utils:assert_eq(target.count, 1, "fire_trigger should skip disabled reactions");
    return true;
  endmethod

  method test_execute_when_clause_gates_effects owner: HACKER
    "Test that when clauses block or allow reaction effects.";
    target = $test_utils:anonymous($root);
    add_property(target, "ready", false, {this.owner, "r"});
    add_property(target, "fired", false, {this.owner, "r"});
    reaction = this:mk('on_ready, "property_is(object: input.This, field: 'ready, value: true)", {{'set, 'fired, true}});
    $reaction:install(target, "ready_reaction", reaction);
    target:fire_trigger('on_ready, ['Actor -> player]);
    $test_utils:assert_false(target.fired, "failed when clause should block effects");
    target.ready = true;
    target:fire_trigger('on_ready, ['Actor -> player]);
    $test_utils:assert_true(target.fired, "successful when clause should allow effects");
    return true;
  endmethod

  method test_trigger_and_action_effects owner: HACKER
    "Test trigger effects chaining into action effects with context bindings.";
    source = $test_utils:anonymous($root);
    receiver = $test_utils:anonymous($root);
    handler = $test_utils:anonymous($root);
    add_property(receiver, "action_seen", false, {this.owner, "r"});
    add_property(receiver, "action_this", #-1, {this.owner, "r"});
    add_property(receiver, "action_actor", #-1, {this.owner, "r"});
    add_verb(handler, {this.owner, "rxd", "action_mark"}, {"this", "none", "this"});
    set_verb_code(handler, "action_mark", {"{target, context} = args;", "target.action_seen = true;", "target.action_this = context['This];", "target.action_actor = context['Actor];", "return true;"});
    $reaction:install(source, "chain_reaction", this:mk('on_first, 0, {{'trigger, receiver, 'on_second}}));
    $reaction:install(receiver, "mark_reaction", this:mk('on_second, 0, {{'action, 'mark, handler}}));
    source:fire_trigger('on_first, ['Actor -> player]);
    $test_utils:assert_true(receiver.action_seen, "triggered action should run");
    $test_utils:assert_eq(receiver.action_this, receiver, "chained trigger should update This binding");
    $test_utils:assert_eq(receiver.action_actor, player, "chained trigger should preserve Actor binding");
    return true;
  endmethod
endobject
