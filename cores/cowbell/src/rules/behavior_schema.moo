object BEHAVIOR_SCHEMA [
  import_export_id -> "behavior_schema",
  import_export_hierarchy -> {"rules"}
]
  name: "Behavior Schema"
  parent: ROOT
  owner: HACKER
  readable: true

  method validate owner: HACKER
    "Validate a definition against an explicit vocabulary. Return it unchanged; grant no authority.";
    "This pure check does not call providers, inspect hosts, mutate state, or suspend.";
    const {definition, vocabulary} = args;
    $behavior_schema:_vocabulary(vocabulary);
    $behavior_schema:_fields(definition, {'name, 'parameters, 'state, 'reactions}, {}, "behavior");
    $behavior_schema:_type(definition['name], 'string, "behavior.name");
    length(definition['name]) > 0 || raise(E_INVARG, "behavior.name: expected a name");
    $behavior_schema:_types(definition['parameters], "behavior.parameters");
    $behavior_schema:_type(definition['state], 'map, "behavior.state");
    let state_types = [];
    for name in (mapkeys(definition['state]))
      $behavior_schema:_name(name, "behavior.state");
      const field = definition['state][name];
      const path = "behavior.state." + tostr(name);
      $behavior_schema:_fields(field, {'type, 'initial}, {}, path);
      $behavior_schema:_type_name(field['type], path + ".type");
      $behavior_schema:_type(field['initial], field['type], path + ".initial");
      state_types[name] = field['type];
    endfor
    $behavior_schema:_type(definition['reactions], 'list, "behavior.reactions");
    length(definition['reactions]) <= 64 || raise(E_QUOTA, "behavior: more than 64 reactions");
    let names = {};
    let nodes = 0;
    for i in [1..length(definition['reactions])]
      const reaction = definition['reactions][i];
      const path = "behavior.reactions[" + tostr(i) + "]";
      $behavior_schema:_fields(reaction,
        {'name, 'trigger, 'condition, 'selection, 'effects, 'enabled}, {}, path);
      $behavior_schema:_name(reaction['name], path + ".name");
      !(reaction['name] in names) || raise(E_INVARG, path + ": duplicate reaction name");
      names = {@names, reaction['name]};
      $behavior_schema:_type(reaction['enabled], 'boolean, path + ".enabled");
      const trigger = $behavior_schema:_entry(vocabulary['triggers], reaction['trigger], path);
      const scope = ['input -> trigger['inputs], 'parameter -> definition['parameters],
        'state -> state_types, 'binding -> []];
      const checked = $behavior_schema:condition(reaction['condition], scope,
        vocabulary['predicates], path + ".condition", 0, nodes);
      nodes = checked['nodes];
      const selection = reaction['selection];
      $behavior_schema:_fields(selection, {'mode, 'limit, 'order}, {}, path + ".selection");
      selection['mode] in {'once, 'one, 'each}
        || raise(E_INVARG, path + ".selection: expected once, one, or each");
      $behavior_schema:_type(selection['limit], 'integer, path + ".selection.limit");
      (selection['limit] > 0 && selection['limit] <= 256)
        || raise(E_INVARG, path + ".selection.limit: expected 1 through 256");
      $behavior_schema:_type(selection['order], 'list, path + ".selection.order");
      let effect_scope = scope;
      if (selection['mode] == 'once)
        length(selection['order]) == 0
          || raise(E_INVARG, path + ": once does not expose selected bindings");
      else
        effect_scope['binding] = checked['bindings];
        let ordered = {};
        for name in (selection['order])
          maphaskey(checked['bindings], name)
            || raise(E_INVARG, path + ": unknown ordering binding " + tostr(name));
          !(name in ordered) || raise(E_INVARG, path + ": repeated ordering binding");
          checked['bindings][name] in {'integer, 'float, 'string, 'symbol, 'object, 'boolean}
            || raise(E_INVARG, path + ": ordering requires scalar bindings");
          ordered = {@ordered, name};
        endfor
        if (selection['mode] == 'each)
          length(ordered) == length(checked['bindings])
            || raise(E_INVARG, path + ": each must order every selected binding");
        endif
      endif
      if (selection['mode] != 'each)
        selection['limit] == 1 || raise(E_INVARG, path + ": once and one require limit 1");
      endif
      nodes = $behavior_schema:effects(reaction['effects], effect_scope,
        vocabulary['actions], path + ".effects", 0, nodes);
    endfor
    return definition;
  endmethod

  method condition owner: HACKER
    "Validate one condition tree and return definitely bound output types and its node count.";
    const {node, scope, predicates, ?path = "condition", ?depth = 0, ?count = 0,
      ?bound_only = false} = args;
    let nodes = $behavior_schema:_node(node, path, depth, count);
    const kind = node['kind];
    let bound = scope['binding];
    if (kind in {'all, 'any})
      $behavior_schema:_fields(node, {'kind, 'children}, {'source}, path);
      $behavior_schema:_type(node['children], 'list, path + ".children");
      let first = true;
      for i in [1..length(node['children])]
        let child_scope = scope;
        kind == 'all && (child_scope['binding] = bound);
        const checked = $behavior_schema:condition(node['children][i], child_scope,
          predicates, path + ".children[" + tostr(i) + "]", depth + 1, nodes, bound_only);
        nodes = checked['nodes];
        if (kind == 'all || first)
          bound = checked['bindings];
        else
          for name in (mapkeys(bound))
            if (!maphaskey(checked['bindings], name))
              bound = mapdelete(bound, name);
            elseif (bound[name] != checked['bindings][name])
              raise(E_INVARG, path + ": alternatives bind different types for " + tostr(name));
            endif
          endfor
        endif
        first = false;
      endfor
    elseif (kind == 'not)
      $behavior_schema:_fields(node, {'kind, 'child}, {'source}, path);
      const checked = $behavior_schema:condition(node['child], scope,
        predicates, path + ".child", depth + 1, nodes, true);
      nodes = checked['nodes];
      equal(checked['bindings], bound)
        || raise(E_INVARG, path + ": negation requires bound inputs and cannot bind outputs");
    elseif (kind == 'compare)
      $behavior_schema:_fields(node, {'kind, 'op, 'left, 'right}, {'source}, path);
      node['op] in {'eq, 'ne, 'lt, 'le, 'gt, 'ge}
        || raise(E_INVARG, path + ": unknown comparison");
      const left_type = $behavior_schema:expression(node['left], scope, path + ".left");
      const right_type = $behavior_schema:expression(node['right], scope, path + ".right");
      $behavior_schema:_compatible(left_type, right_type, path);
      if (!(node['op] in {'eq, 'ne}))
        left_type in {'integer, 'float, 'string}
          || raise(E_INVARG, path + ": ordered comparison requires numbers or strings");
      endif
      nodes = nodes + 2;
    elseif (kind == 'predicate)
      $behavior_schema:_fields(node, {'kind, 'name, 'arguments, 'bind}, {'source}, path);
      const predicate = $behavior_schema:_entry(predicates, node['name], path);
      nodes = $behavior_schema:_arguments(node['arguments], predicate['inputs], scope, path, nodes);
      $behavior_schema:_fields(node['bind], {}, mapkeys(predicate['outputs]), path + ".bind");
      for output in (mapkeys(node['bind]))
        const name = node['bind][output];
        $behavior_schema:_name(name, path + ".bind");
        (!bound_only || maphaskey(bound, name))
          || raise(E_INVARG, path + ": negation cannot introduce " + tostr(name));
        if (maphaskey(bound, name))
          $behavior_schema:_compatible(bound[name], predicate['outputs][output], path);
        endif
        bound[name] = predicate['outputs][output];
      endfor
    else
      raise(E_INVARG, path + ": unknown condition kind " + tostr(kind));
    endif
    nodes <= 1024 || raise(E_QUOTA, path + ": more than 1024 nodes");
    return ['bindings -> bound, 'nodes -> nodes];
  endmethod

  method expression owner: HACKER
    "Return the static type of a literal or a reference in an explicit namespace.";
    const {expression, scope, ?path = "value"} = args;
    typeof(expression) == TYPE_MAP || raise(E_INVARG, path + ": expected a value expression");
    maphaskey(expression, 'kind) || raise(E_INVARG, path + ": missing kind");
    const kind = expression['kind];
    if (kind == 'literal)
      $behavior_schema:_fields(expression, {'kind, 'value}, {'source}, path);
      return $behavior_schema:type_of(expression['value]);
    endif
    kind in {'input, 'parameter, 'state, 'binding}
      || raise(E_INVARG, path + ": unknown value kind");
    $behavior_schema:_fields(expression, {'kind, 'name}, {'source}, path);
    const name = expression['name];
    $behavior_schema:_name(name, path);
    maphaskey(scope[kind], name)
      || raise(E_INVARG, path + ": undeclared " + tostr(kind) + " " + tostr(name));
    return scope[kind][name];
  endmethod

  method effects owner: HACKER
    "Validate ordered actions, declared state writes, and named timers. Return the node count.";
    const {effects, scope, actions, ?path = "effects", ?depth = 0, ?count = 0} = args;
    $behavior_schema:_type(effects, 'list, path);
    let nodes = count;
    for i in [1..length(effects)]
      const effect = effects[i];
      const entry_path = path + "[" + tostr(i) + "]";
      nodes = $behavior_schema:_node(effect, entry_path, depth, nodes);
      const kind = effect['kind];
      if (kind == 'action)
        $behavior_schema:_fields(effect, {'kind, 'name, 'arguments}, {'source}, entry_path);
        const action = $behavior_schema:_entry(actions, effect['name], entry_path);
        nodes = $behavior_schema:_arguments(effect['arguments], action['inputs], scope, entry_path, nodes);
      elseif (kind == 'state)
        $behavior_schema:_fields(effect, {'kind, 'name, 'op, 'value}, {'source}, entry_path);
        const name = effect['name];
        maphaskey(scope['state], name)
          || raise(E_INVARG, entry_path + ": undeclared state " + tostr(name));
        effect['op] in {'set, 'increment, 'decrement}
          || raise(E_INVARG, entry_path + ": unknown state operation");
        const value_type = $behavior_schema:expression(effect['value], scope, entry_path + ".value");
        $behavior_schema:_compatible(value_type, scope['state][name], entry_path);
        if (effect['op] != 'set)
          scope['state][name] in {'integer, 'float}
            || raise(E_INVARG, entry_path + ": arithmetic requires numeric state");
        endif
        nodes = nodes + 1;
      elseif (kind == 'schedule)
        $behavior_schema:_fields(effect, {'kind, 'name, 'seconds, 'effects}, {'source}, entry_path);
        $behavior_schema:_name(effect['name], entry_path + ".name");
        $behavior_schema:expression(effect['seconds], scope, entry_path + ".seconds") == 'integer
          || raise(E_INVARG, entry_path + ": timer seconds must be an integer");
        if (effect['seconds]['kind] == 'literal)
          effect['seconds]['value] >= 0 || raise(E_INVARG, entry_path + ": negative delay");
        endif
        nodes = $behavior_schema:effects(effect['effects], scope, actions,
          entry_path + ".effects", depth + 1, nodes + 1);
      elseif (kind == 'cancel)
        $behavior_schema:_fields(effect, {'kind, 'name}, {'source}, entry_path);
        $behavior_schema:_name(effect['name], entry_path + ".name");
      else
        raise(E_INVARG, entry_path + ": unknown effect kind " + tostr(kind));
      endif
    endfor
    nodes <= 1024 || raise(E_QUOTA, path + ": more than 1024 nodes");
    return nodes;
  endmethod

  method bindings owner: HACKER
    "Validate actual input, parameter, or state values against a type map. Return values unchanged.";
    const {values, types, ?path = "bindings"} = args;
    $behavior_schema:_types(types, path + ".types");
    $behavior_schema:_fields(values, mapkeys(types), {}, path);
    for name in (mapkeys(types))
      $behavior_schema:_type(values[name], types[name], path + "." + tostr(name));
    endfor
    return values;
  endmethod

  method instance owner: HACKER
    "Validate one host's behavior values. Installation authorization remains a separate record.";
    const {instance, vocabulary} = args;
    $behavior_schema:_fields(instance,
      {'definition, 'parameters, 'state, 'revision, 'enabled}, {}, "instance");
    const definition = $behavior_schema:validate(instance['definition], vocabulary);
    $behavior_schema:bindings(instance['parameters], definition['parameters], "instance.parameters");
    let state_types = [];
    for name in (mapkeys(definition['state]))
      state_types[name] = definition['state][name]['type];
    endfor
    $behavior_schema:bindings(instance['state], state_types, "instance.state");
    $behavior_schema:_type(instance['revision], 'integer, "instance.revision");
    instance['revision] > 0 || raise(E_INVARG, "instance.revision: expected positive revision");
    $behavior_schema:_type(instance['enabled], 'boolean, "instance.enabled");
    return instance;
  endmethod

  method type_of owner: HACKER
    "Return a behavioral value type. Opaque MOO values have type any.";
    const {value} = args;
    const types = [TYPE_OBJ -> 'object, TYPE_INT -> 'integer, TYPE_FLOAT -> 'float,
      TYPE_BOOL -> 'boolean, TYPE_STR -> 'string, TYPE_SYM -> 'symbol,
      TYPE_LIST -> 'list, TYPE_MAP -> 'map];
    return `types[typeof(value)] ! E_RANGE => 'any';
  endmethod

  method _vocabulary owner: HACKER
    "Check inert vocabulary records. This does not register or authorize their implementations.";
    const {vocabulary} = args;
    $behavior_schema:_fields(vocabulary, {'triggers, 'predicates, 'actions}, {}, "vocabulary");
    for category in ({'triggers, 'predicates, 'actions})
      $behavior_schema:_type(vocabulary[category], 'map, "vocabulary." + tostr(category));
      for name in (mapkeys(vocabulary[category]))
        const path = "vocabulary." + tostr(category) + "." + tostr(name);
        $behavior_schema:_name(name, path);
        const entry = vocabulary[category][name];
        const fields = category == 'predicates
          ? {'description, 'inputs, 'outputs} | {'description, 'inputs};
        $behavior_schema:_fields(entry, fields, {}, path);
        $behavior_schema:_type(entry['description], 'string, path + ".description");
        $behavior_schema:_types(entry['inputs], path + ".inputs");
        if (category == 'predicates)
          $behavior_schema:_types(entry['outputs], path + ".outputs");
        elseif (category == 'triggers)
          (maphaskey(entry['inputs], 'Self) && entry['inputs]['Self] == 'object)
            || raise(E_INVARG, path + ": triggers require Self of type object");
        endif
      endfor
    endfor
  endmethod

  method _arguments owner: HACKER
    "Validate named argument expressions against an operation's complete input schema.";
    const {arguments, inputs, scope, path, count} = args;
    $behavior_schema:_fields(arguments, mapkeys(inputs), {}, path + ".arguments");
    for name in (mapkeys(inputs))
      const actual = $behavior_schema:expression(arguments[name], scope,
        path + ".arguments." + tostr(name));
      $behavior_schema:_compatible(actual, inputs[name], path + ".arguments." + tostr(name));
    endfor
    return count + length(inputs);
  endmethod

  method _node owner: HACKER
    "Bound tree depth and size, and validate optional source locations.";
    const {node, path, depth, count} = args;
    depth <= 16 || raise(E_QUOTA, path + ": nesting exceeds 16 levels");
    count < 1024 || raise(E_QUOTA, path + ": more than 1024 nodes");
    (typeof(node) == TYPE_MAP && maphaskey(node, 'kind))
      || raise(E_INVARG, path + ": expected a node with kind");
    $behavior_schema:_name(node['kind], path + ".kind");
    return count + 1;
  endmethod

  method _entry owner: HACKER
    "Look up a declared trigger, predicate, or action; reject unknown names.";
    const {entries, name, path} = args;
    $behavior_schema:_name(name, path);
    maphaskey(entries, name) || raise(E_INVARG, path + ": unknown name " + tostr(name));
    return entries[name];
  endmethod

  method _fields owner: HACKER
    "Require exact symbol-keyed records. Validate source positions when the shape permits them.";
    const {record, required, optional, path} = args;
    typeof(record) == TYPE_MAP || raise(E_INVARG, path + ": expected record");
    for name in (required)
      maphaskey(record, name) || raise(E_INVARG, path + ": missing " + tostr(name));
    endfor
    for name in (mapkeys(record))
      (typeof(name) == TYPE_SYM && name in {@required, @optional})
        || raise(E_INVARG, path + ": unknown field " + toliteral(name));
    endfor
    if ('source in optional && maphaskey(record, 'source))
      const source = record['source];
      $behavior_schema:_fields(source, {'start, 'end}, {}, path + ".source");
      $behavior_schema:_type(source['start], 'integer, path + ".source.start");
      $behavior_schema:_type(source['end], 'integer, path + ".source.end");
      (source['start] >= 1 && source['end] >= source['start])
        || raise(E_INVARG, path + ": invalid source positions");
    endif
  endmethod

  method _types owner: HACKER
    "Validate a map from symbolic names to scalar or collection type names.";
    const {types, path} = args;
    typeof(types) == TYPE_MAP || raise(E_INVARG, path + ": expected type map");
    for name in (mapkeys(types))
      $behavior_schema:_name(name, path);
      $behavior_schema:_type_name(types[name], path + "." + tostr(name));
    endfor
  endmethod

  method _name owner: HACKER
    "Require an identifier that authors can spell without MOO symbol quoting.";
    const {name, path} = args;
    typeof(name) == TYPE_SYM || raise(E_INVARG, path + ": expected symbol name");
    match(tostr(name), "^[a-zA-Z_][a-zA-Z0-9_]*$", 1)
      || raise(E_INVARG, path + ": invalid name");
  endmethod

  method _type_name owner: HACKER
    "Reject unknown declared types.";
    const {type, path} = args;
    type in {'object, 'integer, 'float, 'boolean, 'string, 'symbol, 'list, 'map, 'any}
      || raise(E_INVARG, path + ": unknown type");
  endmethod

  method _type owner: HACKER
    "Check an actual value without resolving objects or executing callbacks.";
    const {value, expected, path} = args;
    (expected == 'any || $behavior_schema:type_of(value) == expected)
      || raise(E_INVARG, path + ": expected " + tostr(expected));
  endmethod

  method _compatible owner: HACKER
    "A statically unknown value cannot satisfy a typed operation argument.";
    const {actual, expected, path} = args;
    (expected == 'any || actual == expected)
      || raise(E_INVARG, path + ": expected " + tostr(expected) + ", got " + tostr(actual));
  endmethod
endobject
