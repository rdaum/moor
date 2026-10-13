object RULE_ENGINE [
  import_export_id -> "rule_engine",
  import_export_hierarchy -> {"rules"}
]
  name: "Rule Engine"
  parent: ROOT
  owner: HACKER
  readable: true

  property predicate_sources (owner: ARCH_WIZARD, flags: "r") = [];

  override description (owner: HACKER, flags: "rc") = "Evaluate explicit condition trees over named, typed predicates.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Rules and conditions",
    "A rule is a condition tree with declared input, parameter, and state types.",
    "AND, OR, NOT, and parentheses compose conditions. Unknown predicates and evaluation errors remain errors.",
    "References identify their namespace: input.Actor, parameter.cat, state.counter, or binding.Item.",
    "Quoted strings are literal text. Object references are explicit, such as #65.",
    "Examples: owner_is(object: input.This, owner: input.Accessor)",
    "NOT (input.Actor == parameter.cat) AND state.counter >= 4",
    "occupants(location: input.Location) -> (object: Visitor)",
    "Predicate arguments use names from their schemas. Selected outputs require explicit binding names.",
    "Negation requires bound inputs and cannot introduce new bindings.",
    "Use @set-rule, @show-rule, @clear-rule, and @rules for policy properties.",
    "Core programmers register additional predicates with explicit input and output types.",
    "Read the Cowbell behavior model for the complete data and execution contracts."
  };

  method scope owner: HACKER
    "Construct an explicit static scope. Selected bindings start empty.";
    const {?inputs = [], ?parameters = [], ?state = []} = args;
    return ['input -> inputs, 'parameter -> parameters, 'state -> state, 'binding -> []];
  endmethod

  method policy_scope owner: HACKER
    "Describe inputs supplied by existing policy operations. Unknown slots require an explicit schema.";
    const {property} = args;
    const name = tosym(property);
    let inputs = ['This -> 'object, 'Accessor -> 'object];
    if (name in {'lock_rule, 'unlock_rule})
      inputs['Key] = 'object;
    elseif (name in {'take_rule, 'put_rule})
      inputs['Dobj] = 'object;
    elseif (name == 'feed_rule)
      inputs['Food] = 'object;
    elseif (!(name in {'get_rule, 'drop_rule, 'open_rule, 'close_rule, 'view_rule,
      'read_rule, 'write_rule, 'wear_rule, 'remove_rule, 'use_rule, 'consume_rule, 'pet_rule,
      'sit_rule, 'stand_rule, 'pass_rule}))
      inputs = [];
    endif
    return $rule_engine:scope(inputs);
  endmethod

  method passage_scope owner: HACKER
    "Name passage endpoints explicitly. Key is required only when the condition uses it.";
    return $rule_engine:scope(['Accessor -> 'object, 'From -> 'object, 'To -> 'object,
      'Passage -> 'any, 'Key -> 'object]);
  endmethod

  method trigger_scope owner: HACKER
    "Describe event inputs until operation schemas replace the existing trigger entries.";
    const {trigger, ?state_types = []} = args;
    let inputs = ['This -> 'object, 'Location -> 'object, 'Actor -> 'object];
    if (typeof(trigger) == TYPE_SYM)
      trigger in {'on_take, 'on_put, 'on_get, 'on_drop, 'on_wear, 'on_remove} && (inputs['Item] = 'object);
      trigger in {'on_lock, 'on_unlock} && (inputs['Key] = 'object);
      trigger in {'on_enter, 'on_exit} && (inputs['Who] = 'object);
      trigger in {'on_consume, 'on_empty} && (inputs['Amount] = 'integer);
      trigger == 'on_fed && (inputs['Food] = 'object);
    endif
    return $rule_engine:scope(inputs, [], state_types);
  endmethod

  method schemas owner: HACKER
    "Return pure predicate metadata, without provider identities or authority.";
    let schemas = $world_facts:schemas();
    for name in (mapkeys($rule_engine.predicate_sources))
      schemas[name] = $rule_engine.predicate_sources[name]['schema];
    endfor
    return schemas;
  endmethod

  method register_predicate owner: ARCH_WIZARD
    "Register a typed provider. Only wizards may select executable query implementations.";
    caller_perms().wizard || raise(E_PERM);
    const {name, schema, provider, method} = args;
    $behavior_schema:_name(name, "predicate");
    !maphaskey($world_facts:schemas(), name) || raise(E_INVARG, "Cannot replace a core predicate");
    (typeof(provider) == TYPE_OBJ && valid(provider) && typeof(method) == TYPE_STR)
      || raise(E_INVARG, "Invalid predicate provider");
    $behavior_schema:_vocabulary(['triggers -> [], 'actions -> [], 'predicates -> [name -> schema]]);
    $rule_engine.predicate_sources[name] = ['kind -> 'provider, 'schema -> schema, 'provider -> provider, 'method -> method];
  endmethod

  method register_relation owner: ARCH_WIZARD
    "Name a typed view of one owned relation. Registration grants no permission to read its facts.";
    const {name, relation, columns, schema} = args;
    const principal = caller_perms();
    isa(relation, $relation) || raise(E_INVARG, "Expected a relation");
    (principal == relation.owner || principal.wizard) || raise(E_PERM);
    $behavior_schema:_name(name, "relation query");
    !maphaskey($world_facts:schemas(), name) || raise(E_INVARG, "Cannot replace a core predicate");
    if (maphaskey($rule_engine.predicate_sources, name))
      const previous = $rule_engine.predicate_sources[name];
      (principal.wizard || (previous['kind] == 'relation && previous['relation] == relation))
        || raise(E_PERM, "That query name is already in use");
    endif
    $behavior_schema:_vocabulary(['triggers -> [], 'actions -> [], 'predicates -> [name -> schema]]);
    typeof(columns) == TYPE_LIST || raise(E_TYPE);
    length(columns) > 0 || raise(E_INVARG, "A relation needs at least one field");
    length(columns) == length(schema['inputs]) + length(schema['outputs])
      || raise(E_INVARG, "Each relation field must be declared once");
    let seen = {};
    for column in (columns)
      $behavior_schema:_name(column, "relation field");
      !(column in seen) || raise(E_INVARG, "Repeated relation field");
      seen = {@seen, column};
      (maphaskey(schema['inputs], column) != maphaskey(schema['outputs], column))
        || raise(E_INVARG, "Each relation field must be an input or an output");
    endfor
    $rule_engine.predicate_sources[name] = ['kind -> 'relation, 'schema -> schema,
      'relation -> relation, 'columns -> columns];
  endmethod

  method unregister_predicate owner: ARCH_WIZARD
    "Remove a query as its relation owner or a wizard. Dependent rules then fail validation.";
    const {name} = args;
    const principal = caller_perms();
    if (maphaskey($rule_engine.predicate_sources, name))
      const source = $rule_engine.predicate_sources[name];
      (principal.wizard || (source['kind] == 'relation && principal == source['relation].owner))
        || raise(E_PERM);
      $rule_engine.predicate_sources = mapdelete($rule_engine.predicate_sources, name);
    endif
  endmethod

  method parse_expression owner: HACKER
    "Parse and validate one complete condition using explicitly declared inputs.";
    const {text, ?name = 'parsed_rule, ?scope = $rule_engine:scope()} = args;
    return $rule:mk(name, $condition:parse(text), scope);
  endmethod

  method validate_rule owner: HACKER
    "Validate tree structure, registered predicates, input types, and definite output bindings.";
    const {rule, ?scope = rule.scope} = args;
    try
      (typeof(rule) == TYPE_FLYWEIGHT && rule.delegate == $rule)
        || raise(E_INVARG, "Expected a canonical rule");
      $behavior_schema:_fields(scope, {'input, 'parameter, 'state, 'binding}, {}, "scope");
      for namespace in ({'input, 'parameter, 'state, 'binding})
        $behavior_schema:_types(scope[namespace], "scope." + tostr(namespace));
      endfor
      const checked = $behavior_schema:condition(rule.condition, scope, $rule_engine:schemas());
      return ['valid -> true, 'warnings -> {}, 'bindings -> checked['bindings]];
    except error (E_INVARG, E_QUOTA)
      return ['valid -> false, 'warnings -> {error[2]}, 'bindings -> []];
    endtry
  endmethod

  method decompile_rule owner: HACKER
    "Print the same tree that validation and evaluation consume.";
    const {rule} = args;
    return $condition:print(rule.condition);
  endmethod

  method evaluate owner: ARCH_WIZARD
    "Evaluate a rule under the invoking principal; incomplete exhaustive results remain errors.";
    set_task_perms(caller_perms());
    const {rule, ?inputs = [], ?parameters = [], ?state = [], ?preview = false} = args;
    const validation = $rule_engine:validate_rule(rule);
    validation['valid] || raise(E_INVARG, validation['warnings]:join("; "));
    const environment = ['input -> inputs, 'parameter -> parameters, 'state -> state, 'binding -> []];
    const used = $condition:references(rule.condition);
    for namespace in ({'input, 'parameter, 'state})
      for name in (used[namespace])
        maphaskey(environment[namespace], name) || raise(E_INVARG,
          "Missing " + tostr(namespace) + " " + tostr(name));
        $behavior_schema:_type(environment[namespace][name], rule.scope[namespace][name], tostr(name));
      endfor
    endfor
    const query = {name, values, limit} => $rule_engine:query(name, values, limit, preview);
    const result = $condition:evaluate(rule.condition, environment, query);
    result['complete] || raise(E_QUOTA, "Condition search is incomplete");
    const rows = result['bindings];
    return ['success -> length(rows) > 0, 'bindings -> length(rows) > 0 ? rows[1] | [],
      'alternatives -> length(rows) > 1 ? rows[2..$] | {}, 'warnings -> {}, 'complete -> true];
  endmethod

  method query owner: ARCH_WIZARD
    "Call one registered provider with the caller's permissions and validate its complete row shape.";
    set_task_perms(caller_perms());
    const {name, values, limit, ?preview = false} = args;
    (typeof(limit) == TYPE_INT && limit > 0 && limit <= 256)
      || raise(E_INVARG, "Query result limit must be between 1 and 256");
    const schemas = $rule_engine:schemas();
    maphaskey(schemas, name) || raise(E_INVARG, "Unknown predicate: " + tostr(name));
    const schema = schemas[name];
    $behavior_schema:bindings(values, schema['inputs], "predicate inputs");
    let result;
    if (maphaskey($rule_engine.predicate_sources, name))
      const source = $rule_engine.predicate_sources[name];
      if (source['kind] == 'relation)
        result = $relation:query_rows(source['relation], source['columns], schema, values, limit);
      else
        !preview || raise(E_INVARG, "Preview cannot call the extension condition " + tostr(name) + ".");
        result = source['provider]:(source['method])(name, values, limit);
      endif
    else
      result = $world_facts:query(name, values, limit);
    endif
    $behavior_schema:_fields(result, {'rows, 'complete}, {}, "predicate result");
    $behavior_schema:_type(result['rows], 'list, "predicate rows");
    $behavior_schema:_type(result['complete], 'boolean, "predicate completeness");
    length(result['rows]) <= limit || raise(E_QUOTA, "Predicate exceeded its row limit");
    for row in (result['rows])
      $behavior_schema:bindings(row, schema['outputs], "predicate outputs");
    endfor
    return result;
  endmethod
  method fact_fields owner: HACKER
    "Describe every field required to write one named relation fact.";
    const {name} = args;
    maphaskey($rule_engine.predicate_sources, name) || raise(E_INVARG, "Unknown fact name: " + tostr(name));
    const source = $rule_engine.predicate_sources[name];
    source['kind] == 'relation || raise(E_INVARG, "That name is a condition, not a stored fact.");
    return mapmerge(source['schema]['inputs], source['schema]['outputs]);
  endmethod

  method change_fact owner: ARCH_WIZARD
    "Record or forget an exact typed tuple under the caller's own relation write authority.";
    set_task_perms(caller_perms());
    const {name, values, operation} = args;
    operation in {'record, 'forget} || raise(E_INVARG);
    $behavior_schema:bindings(values, $rule_engine:fact_fields(name), "fact fields");
    const source = $rule_engine.predicate_sources[name];
    const tuple = {values[field] for field in (source['columns])};
    if (operation == 'record)
      return $relation:assert(tuple, source['relation]);
    endif
    return $relation:retract(tuple, source['relation]);
  endmethod
endobject
