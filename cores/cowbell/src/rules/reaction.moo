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

  property action_sources (owner: HACKER, flags: "r") = [];

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
    'schedule,
    'cancel,
    'action,
    'record,
    'forget
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
    'on_passage_open,
    'on_passage_close,
    'on_passage_lock,
    'on_passage_unlock,
    'on_enter,
    'on_exit,
    'on_pet,
    'on_pet_attempt,
    'on_fed,
    'on_cupboard_open,
    'on_couch_intruder,
    'on_kibble_taken,
    'on_consume,
    'on_empty,
    'on_wear,
    'on_remove,
    'on_sit,
    'on_stand,
    'on_sittable_squeeze,
    'on_sittable_dump
  };

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for reactive behaviors on objects.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Reactions",
    "",
    "A reaction responds to an event, such as a cupboard opening, or to a counter crossing a threshold. It can check a condition before it performs its effects.",
    "",
    "Effects run in their listed order. A denied action or an error stops the remaining effects. A false condition skips the reaction.",
    "",
    "## Core programming interface",
    "",
    "`$reaction:mk(trigger, condition, effects, state_types)` constructs a reaction. The condition can be zero or a condition string. The optional state type map describes declared counters and state values.",
    "`$reaction:install(host, name, reaction)` authorizes the definition on an owned object. The property name ends in `_reaction`. Ownership changes and edits invalidate the authorization.",
    "Execution uses the installation owner’s permissions. Event participants do not grant authority. For an action, the `actor` argument identifies the person or owned NPC that will act.",
    "",
    "`$reaction:action_schemas()` lists supported actions and their required named arguments. Action argument values use the same literal, input, state, and binding expressions as conditions.",
    "`$reaction:perform_action(name, arguments)` takes actual values and returns an applied, denied, or error result. A missing argument or unknown action is an error.",
    "Wizards can register a MOO action service with `register_action(name, schema, provider, method)`. The service receives named values and a reaction context. It must check its caller’s authority, preserve the context for chained actions, and return a structured result. Preview must never call the service.",
    "",
    "## State and time",
    "",
    "`$reaction:declare_state(host, name, type, initial, choices)` declares a counter or state value. Choices are optional. `change` checks declared types and choices before it changes a value and evaluates threshold reactions.",
    "`{'schedule, name, seconds, effect}` schedules a named timer. Scheduling the same name replaces its pending task. `{'cancel, name}` cancels it. An object can have at most 16 pending timers, with delays of zero seconds through one day.",
    "Replacing or disabling an installation cancels its pending timers. Re-enabling it does not revive them. A timer rechecks ownership, location, installation, and its condition before it acts.",
    "`timers(host)` lists pending names and due times for the owner. `cancel_timer(host, name)` cancels one timer without changing the reaction.",
    "Scheduling commits earlier effects. A later error cannot undo that commit. Explicit reaction chains stop after four steps. Each reaction has at most 64 immediate effects and 16 levels of nested effects.",
    "",
    "The Cowbell manual’s rule and reaction data reference documents the internal records. The builder authoring commands are separate from this MOO interface."
  };

  method _builtin_action_schemas owner: HACKER
    "Describe supported actions and their named arguments for validation and authoring help.";
    const basic = ['target -> 'object, 'actor -> 'object];
    const item = mapmerge(basic, ['item -> 'object]);
    const key = mapmerge(basic, ['key -> 'object]);
    return [
      'open -> ['description -> "Open a nearby container.", 'inputs -> basic],
      'close -> ['description -> "Close a nearby container.", 'inputs -> basic],
      'lock -> ['description -> "Lock a container with a carried key.", 'inputs -> key],
      'unlock -> ['description -> "Unlock a container with a carried key.", 'inputs -> key],
      'take_from -> ['description -> "Take an item from a container.", 'inputs -> item],
      'put_into -> ['description -> "Put a carried item in a container.", 'inputs -> item],
      'get -> ['description -> "Pick up a nearby item.", 'inputs -> basic],
      'drop -> ['description -> "Drop a carried item.", 'inputs -> basic],
      'sit -> ['description -> "Sit on nearby furniture.", 'inputs -> basic],
      'stand -> ['description -> "Stand up from furniture.", 'inputs -> basic],
      'wear -> ['description -> "Put on a carried wearable item.", 'inputs -> basic],
      'remove -> ['description -> "Take off a worn item.", 'inputs -> basic]
    ];
  endmethod

  method action_schemas owner: HACKER
    "List the argument declarations for built-in actions and registered MOO extensions.";
    let schemas = $reaction:_builtin_action_schemas();
    for name in (mapkeys($reaction.action_sources))
      schemas[name] = $reaction.action_sources[name]['schema];
    endfor
    return schemas;
  endmethod

  method register_action owner: ARCH_WIZARD
    "Register a trusted action service with named arguments. Only wizards can extend the vocabulary.";
    caller_perms().wizard || raise(E_PERM);
    const {name, schema, provider, method} = args;
    $behavior_schema:_name(name, "action");
    !maphaskey($reaction:_builtin_action_schemas(), name)
      || raise(E_INVARG, "Cannot replace a built-in action.");
    $behavior_schema:_vocabulary(['triggers -> [], 'predicates -> [], 'actions -> [name -> schema]]);
    (typeof(provider) == TYPE_OBJ && valid(provider) && typeof(method) == TYPE_STR)
      || raise(E_INVARG, "Invalid action service.");
    respond_to(provider, tosym(method)) || raise(E_VERBNF, "The action service method does not exist.");
    $reaction.action_sources[name] = ['schema -> schema, 'provider -> provider, 'method -> method];
  endmethod

  method unregister_action owner: ARCH_WIZARD
    "Remove a registered action. Reactions that still use its name become invalid.";
    caller_perms().wizard || raise(E_PERM);
    const {name} = args;
    if (maphaskey($reaction.action_sources, name))
      $reaction.action_sources = mapdelete($reaction.action_sources, name);
    endif
  endmethod

  method perform_action owner: ARCH_WIZARD
    "Run a declared action. Return applied, denied, or error; retain error details for the author.";
    set_task_perms(caller_perms());
    const {name, arguments, ?context = []} = args;
    try
      const schemas = $reaction:action_schemas();
      maphaskey(schemas, name) || raise(E_INVARG, "Unknown action: " + tostr(name));
      $behavior_schema:bindings(arguments, schemas[name]['inputs], "action " + tostr(name));
      if (maphaskey($reaction.action_sources, name))
        const service = $reaction.action_sources[name];
        const result = service['provider]:(service['method])(arguments, context);
        (typeof(result) == TYPE_MAP && maphaskey(result, 'status)
          && result['status] in {'applied, 'denied, 'error})
          || raise(E_INVARG, "The action service returned an invalid result.");
        result['status] == 'denied && $behavior_schema:_type(result['reason], 'list, "action denial");
        if (result['status] == 'error)
          const problem = result['error];
          (typeof(problem) == TYPE_LIST && length(problem) >= 3 && typeof(problem[1]) == TYPE_ERR
            && typeof(problem[2]) == TYPE_STR) || raise(E_INVARG, "The action service returned an invalid error.");
        endif
        return result;
      endif
      const {target, who} = {arguments['target], arguments['actor]};
      valid(target) || raise(E_INVARG, "The action's target no longer exists.");
      $root:check_actor(who);
      if (name in {'open, 'close, 'lock, 'unlock, 'take_from, 'put_into})
        const operations = ['take_from -> 'take, 'put_into -> 'put];
        const operation = `operations[name] ! E_RANGE => name';
        const values = mapdelete(mapdelete(arguments, 'target), 'actor);
        return $container:_operate(target, operation, who, values, false, context);
      elseif (name in {'get, 'drop})
        return $thing:transfer(target, name, who, false, context);
      elseif (name in {'wear, 'remove})
        isa(target, $wearable) || raise(E_INVARG, "This action needs a wearable item.");
        const applied = name == 'wear ? target:action_wear(who, context) | target:action_remove(who, context);
        return ['status -> applied ? 'applied | 'denied,
          'reason -> applied ? {} | {"The actor cannot " + tostr(name) + " that item."}];
      else
        isa(target, $sittable) || raise(E_INVARG, "This action needs a seat.");
        const applied = name == 'sit ? target:action_sit(who, context) | target:action_stand(who, context);
        return ['status -> applied ? 'applied | 'denied,
          'reason -> applied ? {} | {"The actor cannot " + tostr(name) + " there."}];
      endif
    except problem (ANY)
      return ['status -> 'error, 'error -> problem];
    endtry
  endmethod

  method _validate_effects owner: ARCH_WIZARD
    "Check action and fact arguments, relation access, and nested timers in the declared scope.";
    set_task_perms(caller_perms());
    const {effects, scope, ?depth = 0, ?parameters = [], ?check_targets = true} = args;
    depth <= 16 || raise(E_QUOTA, "Effects are nested more than 16 levels.");
    length(effects) <= 64 || raise(E_QUOTA, "A reaction has more than 64 effects.");
    for effect in (effects)
      effect.type in $reaction.effect_types || raise(E_INVARG, "Unknown effect: " + tostr(effect.type));
      if (effect.type == 'action)
        const schema = $reaction:action_schemas()[effect.action];
        $behavior_schema:_arguments(effect.arguments, schema['inputs], scope, "action " + tostr(effect.action), 0);
      elseif (effect.type in {'record, 'forget})
        $behavior_schema:_compatible($behavior_schema:expression(effect.relation, scope), 'object, "relation");
        effect.relation['kind] in {'literal, 'parameter}
          || raise(E_INVARG, "Use a relation object or reaction setting.");
        $behavior_schema:_types(effect.fields, "fact fields");
        if (check_targets)
          const target = $condition:relation_target(effect.relation, parameters);
          $relation:schema(target, effect.fields, true);
        endif
        $behavior_schema:_arguments(effect.arguments, effect.fields, scope, "fact fields", 0);
      elseif (effect.type == 'schedule)
        $reaction:_validate_effects({effect.effect}, scope, depth + 1, parameters, check_targets);
      endif
    endfor
  endmethod

  method mk owner: ARCH_WIZARD
    "Create an enabled reaction flyweight from a trigger, optional when clause, and effect specs.";
    "Args: trigger, when_clause (0 or rule string), effects (list of effect specs)";
    "Returns: <REACTION, .trigger, .when, .effects, .enabled, .fired_at>.";
    set_task_perms(caller_perms());
    const {trigger, ?when_clause = 0, ?effects = {}, ?state_types = []} = args;
    "Validate trigger";
    this:validate_trigger(trigger);
    "Parse when clause if provided";
    let parsed_when = 0;
    if (typeof(when_clause) == TYPE_STR && length(when_clause) > 0)
      parsed_when = $rule_engine:parse_expression(when_clause, 'reaction_condition, $rule_engine:trigger_scope(trigger, state_types));
    elseif (typeof(when_clause) != TYPE_INT || when_clause != 0)
      raise(E_INVARG, "Reaction condition must be zero or a nonempty rule string");
    endif
    "Parse and validate effects";
    let parsed_effects = {};
    for effect_spec in (effects)
      parsed_effects = {@parsed_effects, this:parse_effect(effect_spec)};
    endfor
    let scope = $rule_engine:trigger_scope(trigger, state_types);
    if (parsed_when != 0)
      scope['binding] = $rule_engine:validate_rule(parsed_when)['bindings];
    endif
    $reaction:_validate_effects(parsed_effects, scope);
    return <this, .trigger = trigger, .when = parsed_when, .effects = parsed_effects,
      .state_types = state_types, .enabled = true, .fired_at = 0>;
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

  method message_bindings owner: HACKER
    "Describe the participants supplied by each message effect.";
    const {kind} = args;
    let bindings = $sub_utils:standard_bindings("The person who triggered the reaction.");
    if (kind == 'announce)
      bindings['dobj] = ['kind -> 'object, 'description -> "The object with this reaction.",
        'example -> 'host];
    elseif (kind == 'emote)
      bindings['actor]['description] = "The object with this reaction; its name is also prefixed.";
      bindings['actor]['example] = 'host;
      bindings['dobj] = ['kind -> 'object, 'description -> "The person who triggered the reaction.",
        'example -> 'actor];
    elseif (kind == 'tell)
      bindings['iobj] = ['kind -> 'object, 'description -> "The object with this reaction.",
        'example -> 'host];
      bindings['recipient] = ['kind -> 'object, 'description -> "The person receiving this private message.",
        'example -> 'recipient];
    else
      raise(E_INVARG, "This step does not produce a message.");
    endif
    return bindings;
  endmethod

  method parse_effect owner: ARCH_WIZARD
    "Parse one effect, checking names, argument shapes, numeric amounts, and bounded nesting.";
    set_task_perms(caller_perms());
    const {spec, ?depth = 0} = args;
    depth <= 16 || raise(E_QUOTA, "Effects are nested more than 16 levels.");
    (typeof(spec) == TYPE_LIST && length(spec) >= 2) || raise(E_INVARG, "Invalid effect.");
    const kind = spec[1];
    kind in $reaction.effect_types || raise(E_INVARG, "Unknown effect: " + tostr(kind));
    if (kind == 'set)
      const {_, property, value} = spec;
      $behavior_schema:_name(property, "state field");
      return <$reaction, .type = 'set, .prop = property, .value = value>;
    elseif (kind in {'increment, 'decrement})
      const {_, property, ?amount = 1} = spec;
      $behavior_schema:_name(property, "state field");
      typeof(amount) in {TYPE_INT, TYPE_FLOAT} || raise(E_INVARG, "A counter needs a numeric amount.");
      return <$reaction, .type = kind, .prop = property, .by = amount>;
    elseif (kind in {'announce, 'emote})
      const {_, message} = spec;
      const compiled = typeof(message) == TYPE_STR
        ? $sub_utils:compile(message, $reaction:message_bindings(kind)) | message;
      return <$reaction, .type = kind, .msg = compiled>;
    elseif (kind == 'tell)
      const {_, recipient, message} = spec;
      const compiled = typeof(message) == TYPE_STR
        ? $sub_utils:compile(message, $reaction:message_bindings(kind)) | message;
      return <$reaction, .type = 'tell, .target = recipient, .msg = compiled>;
    elseif (kind == 'move)
      const {_, destination} = spec;
      return <$reaction, .type = 'move, .destination = destination>;
    elseif (kind == 'trigger)
      const {_, target, event} = spec;
      $behavior_schema:_name(event, "trigger");
      return <$reaction, .type = 'trigger, .target = target, .event = event>;
    elseif (kind == 'schedule)
      const {_, name, seconds, inner} = spec;
      $behavior_schema:_name(name, "timer name");
      (typeof(seconds) == TYPE_INT && seconds >= 0 && seconds <= 86400)
        || raise(E_INVARG, "A timer must wait between zero seconds and one day.");
      const parsed = $reaction:parse_effect(inner, depth + 1);
      return <$reaction, .type = 'schedule, .name = name, .seconds = seconds, .effect = parsed>;
    elseif (kind == 'cancel)
      const {_, name} = spec;
      $behavior_schema:_name(name, "timer name");
      return <$reaction, .type = 'cancel, .name = name>;
    endif
    if (kind in {'record, 'forget})
      const {_, target, arguments, ?required = 0} = spec;
      const reference = typeof(target) == TYPE_OBJ ? ['kind -> 'literal, 'value -> target] | target;
      const fields = typeof(required) == TYPE_MAP ? required
        | $relation:schema($condition:relation_target(reference, []))['types];
      $behavior_schema:_types(fields, "fact fields");
      $behavior_schema:_fields(arguments, mapkeys(fields), {}, "fact arguments");
      return <$reaction, .type = kind, .relation = reference, .fields = fields, .arguments = arguments>;
    endif
    const {_, name, arguments} = spec;
    const schemas = $reaction:action_schemas();
    maphaskey(schemas, name) || raise(E_INVARG, "Unknown action: " + tostr(name));
    $behavior_schema:_fields(arguments, mapkeys(schemas[name]['inputs]), {}, "action arguments");
    return <$reaction, .type = 'action, .action = name, .arguments = arguments>;
  endmethod

  method _definition owner: ARCH_WIZARD
    "Validate and copy a canonical reaction. Only wizards may select a different validation principal.";
    "Skip target checks for signature verification and editing broken references; installation always checks them.";
    const {value, ?check_targets = true, ?principal = caller_perms()} = args;
    (principal == caller_perms() || caller_perms().wizard) || raise(E_PERM);
    set_task_perms(principal);
    typeof(value) == TYPE_FLYWEIGHT && value.delegate == $reaction || raise(E_INVARG, "Expected a reaction");
    $reaction:validate_trigger(value.trigger);
    (typeof(value.when) == TYPE_INT && value.when == 0) || (typeof(value.when) == TYPE_FLYWEIGHT && value.when.delegate == $rule) || raise(E_INVARG, "Invalid reaction condition");
    typeof(value.effects) == TYPE_LIST || raise(E_INVARG, "Reaction effects must be a list");
    let effects = {};
    for effect in (value.effects)
      const parsed = typeof(effect) == TYPE_LIST ? $reaction:parse_effect(effect) | effect;
      (typeof(parsed) == TYPE_FLYWEIGHT && parsed.delegate == $reaction)
        || raise(E_INVARG, "Invalid reaction effect");
      effects = {@effects, parsed};
    endfor
    const state_types = `value.state_types ! E_PROPNF => []';
    const parameters = `value.parameters ! E_PROPNF => []';
    const parameter_types = `value.parameter_types ! E_PROPNF => []';
    $behavior_schema:bindings(parameters, parameter_types, "reaction settings");
    $behavior_schema:_type(value.enabled, 'boolean, "reaction enabled");
    $behavior_schema:_type(`value.trace ! E_PROPNF => false', 'boolean, "reaction trace");
    const selection = `value.selection ! E_PROPNF => ['mode -> 'one, 'order -> {}, 'limit -> 256]';
    $behavior_schema:_fields(selection, {'mode, 'order, 'limit}, {}, "selection");
    selection['mode] in {'once, 'one, 'each} || raise(E_INVARG, "Choose once, one, or each.");
    typeof(selection['order]) == TYPE_LIST || raise(E_TYPE);
    (typeof(selection['limit]) == TYPE_INT && selection['limit] > 0 && selection['limit] <= 256)
      || raise(E_INVARG, "Match limit must be between 1 and 256.");
    let scope = $rule_engine:trigger_scope(value.trigger, state_types);
    scope['parameter] = parameter_types;
    if (value.when != 0)
      check_targets && $condition:prepare_relations(value.when.condition, parameters);
      const checked = $rule_engine:validate_rule(value.when, scope);
      checked['valid] || raise(E_INVARG, checked['warnings]:join("; "));
      scope['binding] = checked['bindings];
    endif
    for field in (selection['order])
      maphaskey(scope['binding], field) || raise(E_INVARG, "Unknown match name: " + tostr(field));
      scope['binding][field] in {'object, 'integer, 'float, 'string, 'symbol, 'boolean}
        || raise(E_INVARG, "Match ordering needs simple values.");
    endfor
    if (length(selection['order]) > 0)
      length(scope['binding]) == length(selection['order])
        || raise(E_INVARG, "Order every match name to make selection predictable.");
      let seen = {};
      for field in (selection['order])
        !(field in seen) || raise(E_INVARG, "Each match name can appear only once in the order.");
        seen = {@seen, field};
      endfor
    endif
    if (selection['mode] == 'once)
      length(selection['order]) == 0 || raise(E_INVARG, "Select once does not order or expose matches.");
      scope['binding] = [];
    endif
    $reaction:_validate_effects(effects, scope, 0, parameters, check_targets);
    return <$reaction, .trigger = value.trigger, .when = value.when, .effects = effects,
      .state_types = state_types, .parameters = parameters, .parameter_types = parameter_types,
      .selection = selection, .trace = `value.trace ! E_PROPNF => false',
      .enabled = value.enabled, .fired_at = 0>;
  endmethod

  method install owner: ARCH_WIZARD
    "Authorize this exact reaction on one host property under its current owner.";
    const {host, property, value} = args;
    const principal = caller_perms();
    typeof(host) == TYPE_OBJ && valid(host) || raise(E_INVARG);
    principal == host.owner || principal.wizard || raise(E_PERM);
    const name = tostr(property);
    name:ends_with("_reaction") || raise(E_INVARG, "Reaction properties must end with '_reaction'");
    let reaction = $reaction:_definition(value, true, host.owner);
    const declared = $reaction:state_types(host);
    for field in (mapkeys(reaction.state_types))
      (maphaskey(declared, field) && declared[field] == reaction.state_types[field])
        || raise(E_INVARG, "Declare state " + tostr(field) + " on the object before installing this reaction.");
    endfor
    "CBOR preserves anonymous references and value types inside the signed snapshot.";
    const snapshot = {host, name, host.owner, host.owner.wizard, reaction};
    const claims = ['kind -> "cowbell.reaction", 'installation -> encode_base64(encode_cbor(snapshot))];
    reaction.authorization = paseto_make_local(claims);
    set_task_perms(principal);
    for state_name in (mapkeys(reaction.state_types))
      const field_name = tostr(state_name);
      const inherited = `is_clear_property(host, field_name) ! E_INVARG => false';
      if (inherited && index(property_info(host, field_name)[2], "c") > 0)
        host.(field_name) = host.reaction_state_fields[state_name]['initial];
      endif
    endfor
    host.reaction_state_fields = host.reaction_state_fields;
    if ($obj_utils:property_key(host, name) == E_PROPNF)
      add_property(host, name, reaction, {principal, "r"});
    else
      host.(name) = reaction;
    endif
    $reaction:_cancel_installation_timers(host, name);
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
        equal(encode_cbor(definition), encode_cbor($reaction:_definition(reaction, false))) || return 0;
        return owner;
      except (E_INVARG, E_TYPE, E_RANGE, E_PROPNF)
        return 0;
      endtry
    endif
    return 0;
  endmethod

  method declare_state owner: ARCH_WIZARD
    "Declare an owned object's counter or state value, with an optional list of allowed values.";
    const {host, name, type, initial, ?choices = {}} = args;
    const principal = caller_perms();
    (principal == host.owner || principal.wizard) || raise(E_PERM);
    $behavior_schema:_name(name, "state name");
    !(tostr(name) in {"name", "owner", "location", "contents", "programmer", "wizard", "r", "w", "f"})
      || raise(E_PERM, "That name is reserved for an object setting.");
    !tostr(name):starts_with("reaction_") || raise(E_INVARG, "That state name is reserved.");
    $behavior_schema:_type(initial, type, "initial state");
    typeof(choices) == TYPE_LIST || raise(E_TYPE);
    for choice in (choices)
      $behavior_schema:_type(choice, type, "state choice");
    endfor
    $reaction:_check_state_value(initial, ['type -> type, 'choices -> choices]);
    set_task_perms(principal);
    const field = tostr(name);
    if ($obj_utils:property_key(host, field) == E_PROPNF)
      add_property(host, field, initial, {host.owner, "rc"});
    else
      $reaction:_check_state_value(host.(field), ['type -> type, 'choices -> choices]);
    endif
    host.reaction_state_fields[name] = ['type -> type, 'initial -> initial, 'choices -> choices];
    return host.(field);
  endmethod

  method _check_state_value owner: HACKER
    "Require the declared type and, when present, one exact choice.";
    const {value, declaration} = args;
    $behavior_schema:_type(value, declaration['type], "state value");
    length(declaration['choices]) == 0 && return true;
    for choice in (declaration['choices])
      equal(value, choice) && return true;
    endfor
    raise(E_INVARG, "That value is not one of the declared state choices.");
  endmethod

  method state_values owner: ARCH_WIZARD
    "Read declared state using the caller's permissions and validate each current value.";
    set_task_perms(caller_perms());
    const {host} = args;
    let values = [];
    for name in (mapkeys(host.reaction_state_fields))
      const value = host.(tostr(name));
      $reaction:_check_state_value(value, host.reaction_state_fields[name]);
      values[name] = value;
    endfor
    return values;
  endmethod

  method state_types owner: ARCH_WIZARD
    "Return the names and types available to conditions and action arguments on this host.";
    set_task_perms(caller_perms());
    const {host} = args;
    let types = [];
    for name in (mapkeys(host.reaction_state_fields))
      types[name] = host.reaction_state_fields[name]['type];
    endfor
    return types;
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
    name in {"name", "owner", "location", "contents", "programmer", "wizard", "r", "w", "f", "readable", "writable", "fertile", "reaction_state_fields", "reaction_timers", "reaction_reports"} && raise(E_PERM, "Reaction effects cannot change built-in object properties");
    typeof(context) == TYPE_MAP || raise(E_INVARG);
    set_task_perms(principal);
    const old_value = host.(name);
    let operand = value;
    if (operation != 'set)
      (typeof(old_value) in {TYPE_INT, TYPE_FLOAT} && typeof(value) in {TYPE_INT, TYPE_FLOAT})
        || raise(E_INVARG, "Only numeric state can be incremented or decremented.");
      if (typeof(old_value) == TYPE_FLOAT && typeof(value) == TYPE_INT)
        operand = tofloat(value);
      endif
    endif
    const new_value = operation == 'set ? value | (operation == 'increment ? old_value + operand | old_value - operand);
    if (maphaskey(host.reaction_state_fields, tosym(name)))
      $reaction:_check_state_value(new_value, host.reaction_state_fields[tosym(name)]);
    endif
    host.(name) = new_value;
    $reaction:_dispatch(host, 0, context, {tosym(name), old_value, new_value});
    return new_value;
  endmethod

  method _dispatch owner: ARCH_WIZARD
    "Dispatch only after the trigger entry or mutation method authenticates its caller.";
    const source = callers()[1];
    ((source[4] == $root && source[2] == "fire_trigger")
      || (source[4] == $reaction && source[2] == "change")
      || (source[4] == $container && source[2] == "_operate")
      || (source[4] == $passage && source[2] == "operate")
      || (source[4] == $thing && source[2] == "transfer")
      || (source[4] == $wearable && source[2] in {"do_wear", "do_remove"})
      || (source[4] == $henri && source[2] in {"_pet", "_feed"})) || raise(E_PERM);
    const {host, trigger_name, initial_context, ?transition = {}} = args;
    let context = initial_context;
    const chain = `context['_reaction_chain] ! E_RANGE => {}';
    typeof(chain) == TYPE_LIST || raise(E_INVARG, "Invalid reaction chain.");
    const depth = length({frame for frame in (callers())
      if (frame[4] == $reaction && frame[2] == "_dispatch")});
    (length(chain) < 4 && depth < 4)
      || raise(E_QUOTA, "Reaction chain stopped after 4 steps: " + chain:join(" -> "));
    const event = length(transition) == 0 ? tostr(trigger_name) | "change " + tostr(transition[1]);
    context['_reaction_chain] = {@chain, host.name + " / " + event};
    context['This] = host;
    if (!maphaskey(context, 'Location))
      context['Location] = isa(host, $room) ? host | host.location;
    endif
    const definitions = $root:_get_reactions_for(host, $arch_wizard, true);
    const installed = sort(definitions, {tostr(entry[1]) for entry in (definitions)});
    length(installed) <= 64 || raise(E_QUOTA, "This object has more than 64 reactions.");
    for entry in (installed)
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

  method cancel_timer owner: ARCH_WIZARD
    "Cancel a named pending timer on an owned object. Return false when no timer exists.";
    const {host, name} = args;
    const principal = caller_perms();
    (principal == host.owner || principal.wizard) || raise(E_PERM);
    if (!maphaskey(host.reaction_timers, name))
      return false;
    endif
    const timer = host.reaction_timers[name];
    host.reaction_timers = mapdelete(host.reaction_timers, name);
    if (timer['task] != 0)
      `kill_task(timer['task]) ! E_INVARG';
    endif
    return true;
  endmethod

  method timers owner: ARCH_WIZARD
    "List names and due times for an owner's pending timers, without exposing execution records.";
    const {host} = args;
    const principal = caller_perms();
    (principal == host.owner || principal.wizard) || raise(E_PERM);
    let result = [];
    for name in (mapkeys(host.reaction_timers))
      const timer = host.reaction_timers[name];
      result[name] = ['due -> timer['due], 'reaction -> timer['property]];
    endfor
    return result;
  endmethod

  method _cancel_installation_timers owner: ARCH_WIZARD
    "Cancel timers belonging to the reaction being replaced or disabled.";
    const source = callers()[1];
    source[4] == $reaction && source[2] in {"install", "remove"} || raise(E_PERM);
    const {host, property} = args;
    for name in (mapkeys(host.reaction_timers))
      if (host.reaction_timers[name]['property] == tostr(property))
        $reaction:cancel_timer(host, name);
      endif
    endfor
  endmethod

  method _schedule owner: ARCH_WIZARD
    "Replace one named timer. Fork commits earlier effects; no later error can undo that commit.";
    const source = callers()[1];
    source[4] == $reaction && source[2] == "execute_effect" || raise(E_PERM);
    const {host, name, seconds, effect, context, installation, environment} = args;
    const principal = caller_perms();
    (principal == host.owner || principal.wizard) || raise(E_PERM);
    $behavior_schema:_name(name, "timer name");
    (typeof(seconds) == TYPE_INT && seconds >= 0 && seconds <= 86400)
      || raise(E_INVARG, "A timer must wait between zero seconds and one day.");
    $reaction:cancel_timer(host, name);
    length(host.reaction_timers) < 16 || raise(E_QUOTA, "This object already has 16 pending timers.");
    const token = uuid();
    host.reaction_timers[name] = ['token -> token, 'task -> 0, 'due -> time() + seconds,
      'owner -> host.owner, 'wizard -> host.owner.wizard, 'location -> host.location,
      'property -> installation == 0 ? "" | tostr(installation[2]),
      'installation -> installation, 'effect -> effect, 'context -> context, 'environment -> environment];
    fork task (seconds)
      $reaction:_resume_timer(host, name, token);
    endfork
    "A zero-second timer can finish before this parent resumes after fork.";
    if (valid(host) && maphaskey(host.reaction_timers, name)
      && host.reaction_timers[name]['token] == token)
      host.reaction_timers[name]['task] = task;
    endif
    return ['status -> 'scheduled, 'name -> name];
  endmethod

  method _resume_timer owner: ARCH_WIZARD
    "Consume the current timer, recheck its installation and selected values, and report its outcome.";
    const source = callers()[1];
    source[4] == $reaction && source[2] == "_schedule" || raise(E_PERM);
    const {host, name, token} = args;
    valid(host) || return false;
    maphaskey(host.reaction_timers, name) || return false;
    const timer = host.reaction_timers[name];
    timer['token] == token || return false;
    host.reaction_timers = mapdelete(host.reaction_timers, name);
    (host.owner == timer['owner] && host.owner.wizard == timer['wizard]) || return false;
    set_task_perms(host.owner);
    const installation = timer['installation];
    const label = "Timer " + tostr(name) + ": ";
    if (host.location != timer['location])
      $reaction:_timer_trace(host, timer, label + "skipped because the object moved.");
      return false;
    endif
    if (installation != 0 && $reaction:_authority(host, installation[2], installation[3]) != host.owner)
      $reaction:_timer_trace(host, timer, label + "skipped because the installation changed.");
      return false;
    endif
    try
      let environment = timer['environment];
      environment['state] = $reaction:state_values(host);
      if (installation != 0 && installation[3].when != 0)
        const rows = $reaction:_select(installation[3], environment['input]);
        let still_selected = false;
        for row in (rows)
          equal(row, environment['binding]) && (still_selected = true);
        endfor
        if (!still_selected)
          $reaction:_timer_trace(host, timer, label + "skipped because the condition or selected objects changed.");
          return false;
        endif
      endif
      let result;
      if (installation == 0)
        result = $reaction:execute_effect(timer['effect], timer['context], host, 0, environment);
      else
        result = $reaction:_run_installed(host, installation[2], installation[3], timer['context], timer['effect], environment);
      endif
      $reaction:_timer_trace(host, timer, label + "finished.");
      return result;
    except problem (ANY)
      $reaction:_timer_trace(host, timer, label + "failed: " + problem[2]);
      raise(problem[1], problem[2], problem[3]);
    endtry
  endmethod

  method _run_installed owner: ARCH_WIZARD
    "Recheck installation before execution, including after a delayed continuation.";
    const source = callers()[1];
    source[4] == $reaction && source[2] in {"_dispatch", "_resume_timer"} || raise(E_PERM);
    const {host, property, reaction, context, ?effect = 0,
      ?environment = ['input -> context, 'parameter -> [], 'state -> [], 'binding -> []]} = args;
    const principal = $reaction:_authority(host, property, reaction);
    principal != 0 || return false;
    set_task_perms(principal);
    const installation = {host, property, reaction};
    if (effect == 0)
      return reaction:execute(context, installation);
    endif
    return $reaction:execute_effect(effect, context, host, installation, environment);
  endmethod

  method compare owner: HACKER
    "Compare two values using one of the configured comparison operator symbols.";
    {left, op, right} = args;
    if (op == 'eq)
      return equal(left, right);
    elseif (op == 'ne)
      return !equal(left, right);
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
    "Run ordered effects for the selected matches. Trace a skip, completed steps, and a failing step.";
    set_task_perms(caller_perms());
    const {initial_context, ?installation = 0} = args;
    if (installation != 0)
      const source = callers()[1];
      source[4] == $reaction && source[2] == "_run_installed" || raise(E_PERM);
    endif
    const host = initial_context['This];
    const recording = installation != 0 && `this.trace ! E_PROPNF => false';
    let input_parts = {};
    for field in (recording ? mapkeys(initial_context) | {})
      if (!tostr(field):starts_with("_"))
        input_parts = {@input_parts, tostr(field) + "=" + $builder_features:_author_value_text(
          ['kind -> 'literal, 'value -> initial_context[field]])};
      endif
    endfor
    let report = {"Event: " + $builder_features:_author_trigger_text(this.trigger),
      "Inputs: " + input_parts:join(", ")};
    let step = 0;
    try
      if (recording)
        report = {@report, "If: " + (this.when == 0 ? "always" | $builder_features:_author_condition_text(this.when.condition))};
        const state = $reaction:state_values(host);
        for state_name in (mapkeys(state))
          report = {@report, "Current " + tostr(state_name) + ": " + $builder_features:_author_value_text(
            ['kind -> 'literal, 'value -> state[state_name]])};
        endfor
      endif
      const rows = $reaction:_select(this, initial_context);
      report = {@report, "Condition: " + (length(rows) > 0 ? "matched" | "did not match"),
        "Selected matches: " + tostr(length(rows))};
      for row in (rows)
        let environment = ['input -> initial_context, 'parameter -> `this.parameters ! E_PROPNF => []',
          'state -> $reaction:state_values(host), 'binding -> row];
        const context = mapmerge(row, initial_context);
        for selected in (recording ? mapkeys(row) | {})
          report = {@report, "Selected " + tostr(selected) + ": " + $builder_features:_author_value_text(
            ['kind -> 'literal, 'value -> row[selected]])};
        endfor
        for i in [1..length(this.effects)]
          step = i;
          environment['state] = $reaction:state_values(host);
          if (recording)
            report = {@report, "Step " + tostr(i) + ": " + $builder_features:_author_step_text(this.effects[i], environment)};
          endif
          const result = $reaction:execute_effect(this.effects[i], context, host, installation, environment);
          recording && (report = {@report, "Step " + tostr(i) + ": completed"});
          length(report) > 80 && (report = {report[1], @report[$ - 78..$]});
        endfor
      endfor
      recording && $reaction:_record_trace(host, installation[2], report, installation[3]);
      return length(rows) > 0;
    except error (ANY)
      report = {@report, (step == 0 ? "Stopped before the first step: "
        | "Stopped at step " + tostr(step) + ": ") + error[2]};
      recording && $reaction:_record_trace(host, installation[2], report, installation[3]);
      raise(error[1], "Reaction " + (installation == 0 ? "" | tostr(installation[2])) + ": " + error[2], error[3]);
    endtry
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

  method preview_message owner: ARCH_WIZARD
    "Show example wording for a message or scheduled message without running the effect.";
    set_task_perms(caller_perms());
    const {effect, host} = args;
    let message = effect;
    let depth = 0;
    while (message.type == 'schedule)
      depth = depth + 1;
      depth <= 16 || raise(E_INVARG, "Too many nested schedules.");
      message = message.effect;
    endwhile
    !(message.type in {'announce, 'emote, 'tell}) && return {};
    try
      const template = typeof(message.msg) == TYPE_STR ? message.msg | $sub_utils:decompile(message.msg);
      return $sub_utils:preview(template, $reaction:message_bindings(message.type), host, message);
    except error (E_INVARG, E_TYPE, E_PERM, E_VERBNF, E_PROPNF)
      return {"Cannot preview this message: " + error[2]};
    endtry
  endmethod

  method message_event owner: ARCH_WIZARD
    "Build message content for delivery or preview, without sending it.";
    set_task_perms(caller_perms());
    const {kind, actor, host, content, ?recipient = false} = args;
    if (kind == 'announce)
      return $event:mk_info(actor, @content):with_dobj(host);
    elseif (kind == 'emote)
      return $event:mk_emote(host, host.name, " ", @content):with_dobj(actor);
    elseif (kind == 'tell)
      (typeof(recipient) == TYPE_OBJ && valid(recipient)) || raise(E_INVARG, "A tell step needs a recipient.");
      return $event:mk_info(actor, @content):with_iobj(host):with_object_binding('recipient, recipient);
    endif
    raise(E_INVARG, "This step does not produce a message.");
  endmethod

  method execute_effect owner: ARCH_WIZARD
    "Execute a single parsed or raw effect against target using context bindings.";
    "Mutation effects also check threshold reactions on the same target.";
    set_task_perms(caller_perms());
    const {raw_effect, context, target, ?installation = 0,
      ?environment = ['input -> context, 'parameter -> [], 'state -> [], 'binding -> []]} = args;
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
      const event = $reaction:message_event('announce, actor, target, msg);
      const room = `context['Location] ! E_RANGE => target.location';
      if (valid(room) && isa(room, $room))
        room:announce(event);
      endif
    elseif (effect.type == 'emote)
      "Object 'does' something - auto-prefix target name like emote command";
      const msg = this:_resolve_msg(effect.msg, target);
      const event = $reaction:message_event('emote, actor, target, msg);
      const room = `context['Location] ! E_RANGE => target.location';
      if (valid(room) && isa(room, $room))
        room:announce(event);
      endif
    elseif (effect.type == 'tell)
      "Private message to a specific target";
      const recipient = context[effect.target];
      if (valid(recipient))
        const msg = this:_resolve_msg(effect.msg, target);
        const event = $reaction:message_event('tell, actor, target, msg, recipient);
        recipient:tell(event);
      endif
    elseif (effect.type == 'move)
      const dest = typeof(effect.destination) == TYPE_SYM ? context[effect.destination] | effect.destination;
      valid(dest) || raise(E_INVARG, "The movement destination no longer exists.");
      move(target, dest);
    elseif (effect.type == 'trigger)
      "Fire another trigger on another object";
      valid(effect.target) || raise(E_INVARG, "The trigger target no longer exists.");
      effect.target:fire_trigger(effect.event, context);
    elseif (effect.type == 'schedule)
      return $reaction:_schedule(target, effect.name, effect.seconds, effect.effect, context, installation, environment);
    elseif (effect.type == 'cancel)
      return $reaction:cancel_timer(target, effect.name);
    elseif (effect.type in {'action, 'record, 'forget})
      let values = [];
      for name in (mapkeys(effect.arguments))
        values[name] = $condition:resolve(effect.arguments[name], environment);
      endfor
      if (effect.type in {'record, 'forget})
        return $relation:change_fact($condition:resolve(effect.relation, environment), effect.fields, values, effect.type);
      endif
      const result = $reaction:perform_action(effect.action, values, context);
      if (result['status] == 'error)
        const error = result['error];
        raise(error[1], "Action " + tostr(effect.action) + " failed: " + error[2], error[3]);
      endif
      if (result['status] == 'denied)
        raise(E_PERM, "Action " + tostr(effect.action) + " was denied.", result['reason]);
      endif
      return result;
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

  method test_parse_effect_schedule owner: HACKER
    "Test parsing a named timer with nested effect.";
    const effect = this:parse_effect({'schedule, 'reminder, 5, {'announce, "Boom!"}});
    $test_utils:assert_type(effect, TYPE_FLYWEIGHT, "parse_effect() should return a flyweight");
    $test_utils:assert_eq(effect.type, 'schedule, "effect type");
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
    "A chained reaction supplies named action arguments; denial stops its remaining effects.";
    const source = $test_utils:anonymous($root);
    const receiver = $test_utils:anonymous($actor);
    const box = $test_utils:anonymous($container);
    const key = $test_utils:anonymous($thing);
    move(box, receiver);
    move(key, receiver);
    box.locked = true;
    box.unlock_rule = $rule_engine:parse_expression("location_is(object: input.Key, location: input.Accessor)",
      'matching_key, $rule_engine:policy_scope('unlock_rule));
    add_property(receiver, "action_count", 0, {this.owner, "r"});
    const arguments = ['target -> ['kind -> 'literal, 'value -> box],
      'actor -> ['kind -> 'input, 'name -> 'This], 'key -> ['kind -> 'literal, 'value -> key]];
    $reaction:install(source, "chain_reaction", this:mk('on_first, 0, {{'trigger, receiver, 'on_second}}));
    $reaction:install(receiver, "unlock_reaction", this:mk('on_second, 0,
      {{'action, 'unlock, arguments}, {'increment, 'action_count}}));
    source:fire_trigger('on_first, ['Actor -> player]);
    $test_utils:assert_false(box.locked, "Named key and actor arguments reach the operation");
    $test_utils:assert_eq(receiver.action_count, 1, "The following effect runs after success");
    $test_utils:assert_raises(E_PERM, source, "fire_trigger", {'on_first, ['Actor -> player]},
      "An already unlocked box denies the second action");
    $test_utils:assert_eq(receiver.action_count, 1, "Denial stops the remaining effects");
    $test_utils:assert_raises(E_INVARG, $reaction, "mk", {'probe, 0, {{'action, 'missing, []}}},
      "Unknown actions fail while authoring");
    $test_utils:assert_raises(E_INVARG, $reaction, "mk", {'probe, 0,
      {{'action, 'unlock, mapdelete(arguments, 'key)}}}, "A missing key is an authoring error");
    return true;
  endmethod
  method author_scope owner: HACKER
    "Describe the values available to this reaction's conditions and steps.";
    const {reaction} = args;
    let scope = $rule_engine:trigger_scope(reaction.trigger, reaction.state_types);
    scope['parameter] = `reaction.parameter_types ! E_PROPNF => []';
    if (reaction.when != 0)
      scope['binding] = $rule_engine:validate_rule(reaction.when)['bindings];
    endif
    return scope;
  endmethod

  method _select owner: ARCH_WIZARD
    "Evaluate a complete condition and apply an explicit, deterministic match selection.";
    set_task_perms(caller_perms());
    const {reaction, context, ?preview = false} = args;
    const parameters = `reaction.parameters ! E_PROPNF => []';
    const state = $reaction:state_values(context['This]);
    let rows = {[]};
    if (reaction.when != 0)
      const result = $rule_engine:evaluate(reaction.when, context, parameters, state, preview);
      rows = result['success] ? {result['bindings], @result['alternatives]} | {};
    endif
    const selection = `reaction.selection ! E_PROPNF => ['mode -> 'one, 'order -> {}, 'limit -> 256]';
    if (selection['mode] == 'once)
      return length(rows) > 0 ? {[]} | {};
    endif
    if (selection['mode] == 'one && length(rows) > 1)
      raise(E_INVARG, "Expected one match, but found " + tostr(length(rows))
        + ". Narrow the condition, or use select each by NAME.");
    endif
    if (length(rows) > 1)
      length(selection['order]) > 0 || raise(E_INVARG, "Choose the match order with select each by NAME.");
      rows = $reaction:_order_matches(rows, selection['order]);
    endif
    length(rows) <= selection['limit] || raise(E_QUOTA, "More matches than the reaction's limit. Narrow its condition.");
    return selection['mode] == 'one && length(rows) > 0 ? {rows[1]} | rows;
  endmethod

  method _order_matches owner: HACKER
    "Merge-sort at most 256 bindings with exact text ordering and stable scalar keys.";
    const {rows, order} = args;
    length(rows) <= 1 && return rows;
    const midpoint = length(rows) / 2;
    const left = $reaction:_order_matches(rows[1..midpoint], order);
    const right = $reaction:_order_matches(rows[midpoint + 1..$], order);
    let a = 1;
    let b = 1;
    let result = {};
    while (a <= length(left) && b <= length(right))
      let comparison = 0;
      for field in (order)
        const first = left[a][field];
        const second = right[b][field];
        if (!equal(first, second))
          comparison = typeof(first) in {TYPE_STR, TYPE_SYM} ? strcmp(tostr(first), tostr(second))
            | (first < second ? -1 | 1);
          break;
        endif
      endfor
      if (comparison <= 0)
        result = {@result, left[a]};
        a = a + 1;
      else
        result = {@result, right[b]};
        b = b + 1;
      endif
    endwhile
    return {@result, @left[a..$], @right[b..$]};
  endmethod

  method remove owner: ARCH_WIZARD
    "Remove a local reaction or hide an inherited one on this object, then cancel its timers.";
    const {host, property} = args;
    const principal = caller_perms();
    (principal == host.owner || principal.wizard) || raise(E_PERM, "You do not own that object.");
    const name = tostr(property);
    name:ends_with("_reaction") || raise(E_INVARG);
    typeof(host.(name)) == TYPE_FLYWEIGHT && host.(name).delegate == $reaction || raise(E_INVARG);
    set_task_perms(principal);
    if (name in properties(host))
      delete_property(host, name);
    else
      host.(name) = 0;
    endif
    $reaction:_cancel_installation_timers(host, name);
  endmethod

  method _record_trace owner: ARCH_WIZARD
    "Store the latest bounded execution report. Only the canonical executor can write it.";
    const source = callers()[1];
    source[4] == $reaction && source[2] in {"execute", "_timer_trace"} || raise(E_PERM);
    const {host, property, report, definition} = args;
    const principal = caller_perms();
    (host.owner == principal || principal.wizard) || return false;
    let reports = host.reaction_reports;
    if (length(reports) >= 64 && !maphaskey(reports, property))
      reports = [];
    endif
    reports[tostr(property)] = ['lines -> report, 'definition -> definition];
    host.reaction_reports = reports;
    return true;
  endmethod

  method report owner: ARCH_WIZARD
    "Read one execution report as the host's owner or a wizard.";
    const {host, property} = args;
    const principal = caller_perms();
    (host.owner == principal || principal.wizard) || raise(E_PERM, "Only the owner can read execution reports.");
    const record = `host.reaction_reports[tostr(property)] ! E_RANGE => 0';
    record == 0 && return {};
    const current = `host.(tostr(property)) ! E_PROPNF => 0';
    return equal(current, record['definition]) ? record['lines]
      | {"This report is from an earlier version of the reaction.", @record['lines]};
  endmethod
  method _timer_trace owner: ARCH_WIZARD
    "Append a timer outcome to its installed reaction's report without exposing another owner's data.";
    const source = callers()[1];
    source[4] == $reaction && source[2] == "_resume_timer" || raise(E_PERM);
    const {host, timer, line} = args;
    const installation = timer['installation];
    installation == 0 && return;
    `installation[3].trace ! E_PROPNF => false' || return;
    host.owner == timer['owner] || return;
    const old = `host.reaction_reports[tostr(installation[2])] ! E_RANGE => 0';
    const lines = old != 0 && equal(old['definition], installation[3]) ? old['lines] | {};
    set_task_perms(host.owner);
    $reaction:_record_trace(host, installation[2], {@lines[max(1, length(lines) - 78)..$], line}, installation[3]);
  endmethod
endobject
