object WORLD_FACTS [
  import_export_id -> "world_facts",
  import_export_hierarchy -> {"rules"}
]
  name: "World Facts"
  parent: ROOT
  owner: HACKER
  readable: true

  method schemas owner: HACKER
    "Declare the read-only world predicates and their named inputs and outputs.";
    return [
      'is -> ['description -> "Compare two exact values.",
        'inputs -> ['left -> 'any, 'right -> 'any], 'outputs -> []],
      'isa -> ['description -> "Check an object's prototype ancestry.",
        'inputs -> ['object -> 'object, 'prototype -> 'object], 'outputs -> []],
      'owner_is -> ['description -> "Check an object's owner.",
        'inputs -> ['object -> 'object, 'owner -> 'object], 'outputs -> []],
      'location_is -> ['description -> "Check an object's location.",
        'inputs -> ['object -> 'object, 'location -> 'object], 'outputs -> []],
      'nearby -> ['description -> "Check whether an object is at a supplied location.",
        'inputs -> ['object -> 'object, 'location -> 'object], 'outputs -> []],
      'contains -> ['description -> "Check whether a container directly contains an item.",
        'inputs -> ['container -> 'object, 'item -> 'object], 'outputs -> []],
      'is_wizard -> ['description -> "Check an actor's wizard flag.",
        'inputs -> ['actor -> 'object], 'outputs -> []],
      'is_programmer -> ['description -> "Check an actor's programmer flag.",
        'inputs -> ['actor -> 'object], 'outputs -> []],
      'is_builder -> ['description -> "Check an actor's builder flag.",
        'inputs -> ['actor -> 'object], 'outputs -> []],
      'is_open -> ['description -> "Check a container's open state.",
        'inputs -> ['object -> 'object], 'outputs -> []],
      'is_locked -> ['description -> "Check a container's locked state.",
        'inputs -> ['object -> 'object], 'outputs -> []],
      'is_portable -> ['description -> "Check an item's portable state.",
        'inputs -> ['object -> 'object], 'outputs -> []],
      'sitting -> ['description -> "Check whether an actor is on a seat.",
        'inputs -> ['actor -> 'object, 'seat -> 'object], 'outputs -> []],
      'property_is -> ['description -> "Compare a readable property with an exact value.",
        'inputs -> ['object -> 'object, 'field -> 'symbol, 'value -> 'any], 'outputs -> []],
      'property_at_least -> ['description -> "Compare a readable integer property with a minimum.",
        'inputs -> ['object -> 'object, 'field -> 'symbol, 'minimum -> 'integer], 'outputs -> []],
      'occupants -> ['description -> "Select direct contents of one readable location.",
        'inputs -> ['location -> 'object], 'outputs -> ['object -> 'object]]
    ];
  endmethod

  method query owner: ARCH_WIZARD
    "Read a bounded world scope with the caller's permissions. No object predicate callbacks.";
    set_task_perms(caller_perms());
    const {name, values, limit} = args;
    const schema = $world_facts:schemas()[name];
    $behavior_schema:bindings(values, schema['inputs]);
    for key in (mapkeys(schema['inputs]))
      if (schema['inputs][key] == 'object)
        valid(values[key]) || raise(E_INVARG, "Invalid object argument " + tostr(key));
      endif
    endfor
    let matches = false;
    if (name == 'is)
      matches = equal(values['left], values['right]);
    elseif (name == 'isa)
      matches = isa(values['object], values['prototype]);
    elseif (name == 'owner_is)
      matches = values['object].owner == values['owner];
    elseif (name in {'location_is, 'nearby})
      matches = values['object].location == values['location];
    elseif (name == 'contains)
      matches = values['item].location == values['container];
    elseif (name in {'is_wizard, 'is_programmer, 'is_builder})
      const fields = ['is_wizard -> "wizard", 'is_programmer -> "programmer", 'is_builder -> "is_builder"];
      matches = !!values['actor].(fields[name]);
    elseif (name in {'is_open, 'is_locked, 'is_portable})
      const fields = ['is_open -> "open", 'is_locked -> "locked", 'is_portable -> "portable"];
      matches = !!values['object].(fields[name]);
    elseif (name == 'sitting)
      matches = !!(values['actor] in values['seat].sitting);
    elseif (name == 'property_is)
      matches = equal(values['object].(tostr(values['field])), values['value]);
    elseif (name == 'property_at_least)
      const value = values['object].(tostr(values['field]));
      typeof(value) == TYPE_INT || raise(E_TYPE, "Expected an integer property");
      matches = value >= values['minimum];
    elseif (name == 'occupants)
      const contents = values['location].contents;
      return ['rows -> {['object -> item] for item in (contents[1..min(length(contents), limit)])},
        'complete -> length(contents) <= limit];
    endif
    return ['rows -> matches ? {[]} | {}, 'complete -> true];
  endmethod
endobject
