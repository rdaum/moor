object BEHAVIOR_SCENARIOS
  name: "Behavior Contract Scenarios"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method vocabulary owner: HACKER
    "An explicit cupboard vocabulary for pure validation, without installed provider authority.";
    return [
      'triggers -> ['opened -> ['description -> "A successful opening.",
        'inputs -> ['Self -> 'object, 'Actor -> 'object, 'Location -> 'object]]],
      'predicates -> [
        'nearby -> ['description -> "Check an object's current location.",
          'inputs -> ['object -> 'object, 'location -> 'object], 'outputs -> []],
        'occupants -> ['description -> "Select occupants of a location.",
          'inputs -> ['location -> 'object], 'outputs -> ['object -> 'object]]],
      'actions -> [
        'stand -> ['description -> "Ask an actor to leave its seat.",
          'inputs -> ['actor -> 'object]],
        'announce -> ['description -> "Send a literal message to a location.",
          'inputs -> ['location -> 'object, 'text -> 'string]]]
    ];
  endmethod

  method cupboard owner: HACKER
    "A complete definition: a nearby parameterized cat responds to the cupboard opening.";
    return ['name -> "Cupboard attracts a cat", 'parameters -> ['cat -> 'object],
      'state -> ['openings -> ['type -> 'integer, 'initial -> 0]],
      'reactions -> {['name -> 'greet, 'trigger -> 'opened, 'enabled -> true,
        'condition -> ['kind -> 'predicate, 'name -> 'nearby,
          'arguments -> ['object -> ['kind -> 'parameter, 'name -> 'cat],
            'location -> ['kind -> 'input, 'name -> 'Location]], 'bind -> []],
        'selection -> ['mode -> 'once, 'limit -> 1, 'order -> {}],
        'effects -> {
          ['kind -> 'state, 'name -> 'openings, 'op -> 'increment,
            'value -> ['kind -> 'literal, 'value -> 1]],
          ['kind -> 'action, 'name -> 'stand,
            'arguments -> ['actor -> ['kind -> 'parameter, 'name -> 'cat]]],
          ['kind -> 'action, 'name -> 'announce,
            'arguments -> ['location -> ['kind -> 'input, 'name -> 'Location],
              'text -> ['kind -> 'literal, 'value -> "The cat looks interested."]]]}
      ]}];
  endmethod

  method _invalid owner: HACKER
    "Require a definition error with the field or rule that failed.";
    const {definition, fragment, ?error_code = E_INVARG} = args;
    try
      $behavior_schema:validate(definition, this:vocabulary());
    except error (E_INVARG, E_QUOTA)
      $test_utils:assert_eq(error[1], error_code, "Error kind");
      $test_utils:assert_true(index(error[2], fragment, 1),
        "Expected " + fragment + " in " + error[2]);
      return;
    endtry
    raise(E_ASSERT, "Expected invalid behavior: " + fragment);
  endmethod

  method test_cupboard_definition_and_instances owner: HACKER
    "Definitions are inert data; instances have separate typed parameters and state.";
    const definition = this:cupboard();
    const vocabulary = this:vocabulary();
    $test_utils:assert_eq($behavior_schema:validate(definition, vocabulary), definition,
      "Validation preserves the definition without executing effects");
    let instance = ['definition -> definition, 'parameters -> ['cat -> #-1],
      'state -> ['openings -> 0], 'revision -> 1, 'enabled -> true];
    $test_utils:assert_eq($behavior_schema:instance(instance, vocabulary), instance,
      "Shape validation does not resolve or authorize object references");
    instance['parameters]['cat] = "Henri";
    $test_utils:assert_raises(E_INVARG, $behavior_schema, "instance", {instance, vocabulary},
      "Object parameters are references, not implicit name matches");
    instance['parameters]['cat] = #-1;
    instance['state]['openings] = false;
    $test_utils:assert_raises(E_INVARG, $behavior_schema, "instance", {instance, vocabulary},
      "Boolean is not an integer state value");
    return true;
  endmethod

  method test_definition_errors_identify_fields owner: HACKER
    "Misspelled fields, operations, and undeclared references must fail during authoring.";
    let definition = this:cupboard();
    definition['authority] = #2;
    this:_invalid(definition, "unknown field");
    definition = this:cupboard();
    definition['reactions][1]['trigger] = 'openned;
    this:_invalid(definition, "unknown name");
    definition = this:cupboard();
    definition['reactions][1]['effects][2]['arguments] = [];
    this:_invalid(definition, "missing actor");
    definition = this:cupboard();
    definition['reactions][1]['effects][1]['name] = 'wizard;
    this:_invalid(definition, "undeclared state");
    definition = this:cupboard();
    definition['reactions][1]['condition]['arguments]['object]['name] = 'dog;
    this:_invalid(definition, "undeclared parameter");
    definition = this:cupboard();
    definition['reactions] = {definition['reactions][1], definition['reactions][1]};
    this:_invalid(definition, "duplicate reaction");
    return true;
  endmethod

  method test_bindings_selection_and_negation owner: HACKER
    "Only definite outputs reach effects; negation cannot introduce hidden outputs.";
    let definition = this:cupboard();
    const query = ['kind -> 'predicate, 'name -> 'occupants,
      'arguments -> ['location -> ['kind -> 'input, 'name -> 'Location]],
      'bind -> ['object -> 'Visitor]];
    definition['reactions][1]['condition] = query;
    definition['reactions][1]['effects] = {['kind -> 'action, 'name -> 'stand,
      'arguments -> ['actor -> ['kind -> 'binding, 'name -> 'Visitor]]]};
    this:_invalid(definition, "undeclared binding");
    definition['reactions][1]['selection] = ['mode -> 'each, 'limit -> 8, 'order -> {'Visitor}];
    $behavior_schema:validate(definition, this:vocabulary());
    definition['reactions][1]['condition] = ['kind -> 'any,
      'children -> {query, ['kind -> 'all, 'children -> {}]}];
    this:_invalid(definition, "unknown ordering binding");
    definition['reactions][1]['condition] = ['kind -> 'not,
      'child -> definition['reactions][1]['condition]];
    this:_invalid(definition, "negation cannot introduce");
    definition = this:cupboard();
    definition['reactions][1]['condition] = ['kind -> 'not,
      'child -> definition['reactions][1]['condition]];
    $behavior_schema:validate(definition, this:vocabulary());
    return true;
  endmethod

  method test_state_timers_and_limits owner: HACKER
    "Timers retain declared effects and input types; definitions have finite structural limits.";
    let definition = this:cupboard();
    definition['reactions][1]['effects] = {
      ['kind -> 'schedule, 'name -> 'reset, 'seconds -> ['kind -> 'literal, 'value -> 5],
        'effects -> {['kind -> 'state, 'name -> 'openings, 'op -> 'set,
          'value -> ['kind -> 'literal, 'value -> 0]]}],
      ['kind -> 'cancel, 'name -> 'reset]};
    $behavior_schema:validate(definition, this:vocabulary());
    definition['reactions][1]['effects][1]['seconds]['value] = -1;
    this:_invalid(definition, "negative delay");
    definition = this:cupboard();
    let condition = definition['reactions][1]['condition];
    for i in [1..18]
      condition = ['kind -> 'not, 'child -> condition];
    endfor
    definition['reactions][1]['condition] = condition;
    this:_invalid(definition, "nesting exceeds", E_QUOTA);
    definition = this:cupboard();
    definition['reactions][1]['condition]['source] = ['start -> 9, 'end -> 2];
    this:_invalid(definition, "invalid source positions");
    return true;
  endmethod
endobject
