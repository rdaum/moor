object AGENT_BUILDING_TOOLS [
  import_export_id -> "agent_building_tools",
  import_export_hierarchy -> {"llm"}
]
  name: "Agent Building Tools"
  parent: LLM_TOOL_PROVIDER
  owner: ARCH_WIZARD
  readable: true

  property guide (owner: ARCH_WIZARD, flags: "rc") = "SPATIAL CONCEPTS:\n\n1) AREAS are organizational containers (like buildings or zones) that group related rooms together. Areas have object IDs like #38.\n\n2) ROOMS are individual locations within an area. Rooms have object IDs like #12 or #50.\n\n3) The hierarchy is: AREA contains ROOMS, not the other way around.\n\n4) When a user says 'build rooms in the hotel lobby area', they mean build rooms in the SAME AREA that contains the hotel lobby room, NOT inside the lobby room itself.\n\n5) ALWAYS use object numbers (like #38 or #50) when referencing specific objects to avoid ambiguity. NEVER use names alone.\n\nOBJECT PROTOTYPES:\n\nThe system provides prototype objects that serve as templates for creating new objects. Use the 'list_prototypes' tool to see available prototypes like $room (rooms), $thing (generic objects), $wearable (items that can be worn), and $area (organizational containers). When creating objects, choose the appropriate prototype as the parent - for example, use $wearable for items like hats or tools, $thing for furniture or decorations, and $room for new locations.\n\nMOVING OBJECTS:\n\nUse the 'move_object' tool to relocate objects between locations. You can move objects to rooms, players, or containers. This is useful for placing furniture in rooms, giving items to players, or organizing objects. You must own the object or be a wizard to move it.\n\nRULES AND REACTIONS:\n\nRules check whether an operation is allowed. Reactions respond to an event with a condition and actions.\n\nUse list_rules and show_rule to inspect object policies. Use set_rule to edit them. A condition such as 'input.Key == #123' compares the supplied key with one specific object. Quoted text remains text.\n\nConditions support AND, OR, NOT, and parentheses. Predicates take named arguments, for example 'owner_is(object: input.This, owner: input.Accessor)'. The operation determines which inputs are available. Unknown inputs or predicates are errors.\n\nA query can select values for later conditions: 'occupants(location: input.Location) -> (object: Visitor) AND binding.Visitor != input.Actor'. Use doc_lookup(\"$rule_engine\") for the condition syntax.\n\nCONSTRUCTION DEFAULTS:\n\nWhen building rooms, if no area is specified, rooms are created in the user's current area automatically - you do NOT need to specify an area unless the user wants rooms in a different area. The 'area' parameter for build_room is optional and defaults to the user's current area.\n\nSUBSTITUTION TEMPLATES:\n\nUse $sub/$sub_utils syntax for dynamic messages: {n/nc} actor, {d/dc} dobj, {i}, {t}, {l}; articles {a d}/{an d}/{the d} render article + noun; pronouns {s/o/p/q/r} with _dobj/_iobj variants; self alternation {you|they} auto-picks perspective; verbs conjugate with be/have/look.\n\nALWAYS use self-alternation for verbs that differ by person (e.g., {set|sets}, {place|places}) so the actor sees second-person grammar.\n\nBefore crafting templates, use doc_lookup(\"$sub_utils\") to recall article rules and binding variants.";

  override description (owner: ARCH_WIZARD, flags: "rc") = "Shared tool definitions and handlers for AI agents that build and manipulate the world. Used by both in-world wearables (like $architects_compass) and external MCP agents.";

  method get_tools owner: ARCH_WIZARD
    "Compose domain catalogs for in-world and external agents.";
    return $llm_tool_catalog:compose($llm_inspection_tools:get_tools(), $llm_programming_tools:get_tools(), $llm_building_tools:get_tools(), $llm_behavior_tools:get_tools(), $llm_help_tools:get_tools());
  endmethod

  method test_core_building_tool_metadata owner: ARCH_WIZARD
    "Core building tools should expose real prototype and object metadata.";
    let audit = 0;
    let details = 0;
    let message = 0;
    let messages = 0;
    let rules = 0;
    const prototypes = $llm_inspection_tools:list_prototypes([], player);
    $test_utils:assert_true(index(prototypes, "$thing") > 0, "list_prototypes should include $thing");
    $test_utils:assert_true(index(prototypes, $thing.description) > 0, "list_prototypes should include prototype descriptions");
    let created = #-1;
    try
      message = $llm_building_tools:create_object(["parent" -> "$thing", "name" -> "tool metadata probe:tool-probe"], $hacker);
      $test_utils:assert_true(index(message, "Created \"tool metadata probe\"") > 0, "create_object should report created object");
      created = $match:match_object("tool-probe", $hacker);
      $test_utils:assert_type(created, TYPE_OBJ, "created object should be matchable by alias");
      $test_utils:assert_true(valid(created), "created object should be valid");
      details = $llm_inspection_tools:inspect_object(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(details, "Name: tool metadata probe") > 0, "inspect_object should include real name");
      $test_utils:assert_true(index(details, "Description:") > 0, "inspect_object should include description field");
      $test_utils:assert_true(index(details, "Parent:") > 0, "inspect_object should include parent metadata");
      message = $llm_building_tools:rename_object(["object" -> created, "name" -> "renamed metadata probe:renamed-probe"], $hacker);
      $test_utils:assert_true(index(message, "Renamed \"tool metadata probe\"") > 0, "rename_object should report the real previous name");
      $test_utils:assert_true(index(message, "to \"renamed metadata probe\"") > 0, "rename_object should report the new name");
      message = $llm_building_tools:describe_object(["object" -> created, "description" -> "A probe with renamed metadata."], $hacker);
      $test_utils:assert_true(index(message, "Set description of \"renamed metadata probe\"") > 0, "describe_object should report the real object name");
      message = $llm_building_tools:move_object(["object" -> created, "destination" -> $hacker], $hacker);
      $test_utils:assert_true(index(message, "Moved \"renamed metadata probe\"") > 0, "move_object should report the real object name");
      message = $llm_building_tools:set_integrated_description(["object" -> created, "integrated_description" -> "a renamed probe is here"], $hacker);
      $test_utils:assert_true(index(message, "Set integrated description of \"renamed metadata probe\"") > 0, "set_integrated_description should report the real object name");
      audit = $llm_inspection_tools:audit_owned([], $hacker);
      $test_utils:assert_true(index(audit, "renamed metadata probe") > 0, "audit_owned should list real object names");
      add_property(created, "probe_rule", 0, {$hacker, "rc"});
      add_property(created, "probe_msg", "Probe message.", {$hacker, "rc"});
      rules = $llm_behavior_tools:list_rules(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(rules, "probe_rule: (no rule set)") > 0, "list_rules should read real rule property values");
      messages = $llm_behavior_tools:list_messages(["object" -> tostr(created)], $hacker);
      $test_utils:assert_true(index(messages, "probe_msg: \"Probe message.\"") > 0, "list_messages should read real message property values");
      message = $llm_behavior_tools:set_message_template(["object" -> tostr(created), "property" -> "probe_msg", "template" -> "Updated probe message."], $hacker);
      $test_utils:assert_true(index(message, "Set " + tostr(created) + ".probe_msg") > 0, "set_message_template should report scalar message update");
      $test_utils:assert_true(index($sub_utils:decompile(created.probe_msg), "Updated probe message") > 0, "set_message_template should store compiled scalar message");
      message = $llm_behavior_tools:add_message_template(["object" -> tostr(created), "property" -> "probe_msgs", "template" -> "First bag message."], $hacker);
      $test_utils:assert_true(index(message, "Added message to " + tostr(created) + ".probe_msgs") > 0, "add_message_template should report bag append");
      $test_utils:assert_eq(length(created.probe_msgs:entries()), 1, "add_message_template should create a one-entry bag");
      message = $llm_behavior_tools:delete_message_template(["object" -> tostr(created), "property" -> "probe_msgs", "index" -> 1], $hacker);
      $test_utils:assert_true(index(message, "Deleted message 1 from " + tostr(created) + ".probe_msgs") > 0, "delete_message_template should report bag removal");
      $test_utils:assert_eq(length(created.probe_msgs:entries()), 0, "delete_message_template should remove the bag entry");
    finally
      $test_utils:destroy_if_valid(created);
    endtry
    return true;
  endmethod

  method test_code_property_tool_actor_permissions owner: ARCH_WIZARD
    "Private code and property inspection require the requesting actor's ordinary authority.";
    const target = create($thing, $hacker, 2);
    try
      target.r = 0;
      add_property(target, "agent_building_private_probe", "secret", {$hacker, ""});
      add_verb(target, {$hacker, "xd", "agent_building_private_verb"}, {"this", "none", "this"});
      set_verb_code(target, "agent_building_private_verb", {"return \"secret\";"});
      for entry in ({{"get_property", ["object" -> target, "property" -> "agent_building_private_probe"]}, {"get_verb_code", ["object" -> target, "verb" -> "agent_building_private_verb"]}, {"list_verbs", ["object" -> target]}, {"list_properties", ["object" -> target]}})
        const {endpoint, values} = entry;
        let denied = false;
        try
          $llm_inspection_tools:(endpoint)(values, $test_player);
        except (E_PERM)
          denied = true;
        endtry
        $test_utils:assert_true(denied, endpoint + " must not mint private target grants");
      endfor
    finally
      $test_utils:destroy_if_valid(target);
    endtry
    return true;
  endmethod

  method test_navigation_tool_metadata owner: ARCH_WIZARD
    "Navigation tools should expose real room, area, and passage metadata.";
    let details = 0;
    let map = 0;
    let passage = 0;
    let result = 0;
    let route = 0;
    let updated_passage = 0;
    const area = $area:create(true);
    const other_area = $area:create(true);
    const r1 = $room:create(true);
    const r2 = $room:create(true);
    const actor = $root:create(true);
    try
      actor.name = "Tool Test Actor";
      area.name = "Tool Test Area";
      r1.name = "Tool Test Start";
      r2.name = "Tool Test End";
      move(r1, area);
      move(r2, area);
      move(actor, r1);
      passage = <$passage, .side_a_room = r1, .side_a_label = "east", .side_b_room = r2, .side_b_label = "west", .is_open = true>;
      passage = area:set_passage(r1, r2, passage);
      let invalid_template = false;
      try
        $llm_building_tools:set_passage_description(["direction" -> "east", "description" -> "broken {template", "format" -> "template", "source_room" -> r1], $arch_wizard);
      except (E_INVARG)
        invalid_template = true;
      endtry
      $test_utils:assert_true(invalid_template, "set_passage_description should reject malformed templates");
      result = $llm_building_tools:set_passage_description(["direction" -> "east", "description" -> "{nc} heads east.", "format" -> "template", "source_room" -> r1], $arch_wizard);
      $test_utils:assert_true(index(result, "Set description for 'east' passage") > 0, "set_passage_description should report success");
      updated_passage = area:passage_for(r1, r2);
      $test_utils:assert_eq(updated_passage.side_a_description, "{nc} heads east.", "The tool retains editable source");
      $test_utils:assert_eq(updated_passage.side_a_description_kind, 'template, "The tool stores the explicit format");
      map = $llm_inspection_tools:area_map([], actor);
      $test_utils:assert_true(index(map, "Current Location: Tool Test Start") > 0, "area_map should include current room name: " + toliteral(map));
      $test_utils:assert_true(index(map, "Area: Tool Test Area") > 0, "area_map should include area name");
      $test_utils:assert_true(index(map, "Tool Test End") > 0, "area_map should include rooms in area");
      move(r2, other_area);
      route = $llm_inspection_tools:mcp_find_route(["to_room" -> "Tool Test End"], actor);
      $test_utils:assert_true(index(route, "Route from Tool Test Start") > 0, "find_route should include start room name");
      $test_utils:assert_true(index(route, "Go east to Tool Test End") > 0, "find_route should include passage label and destination");
      route = $llm_inspection_tools:mcp_find_route(
        ["from_room" -> "Tool Test End", "to_room" -> "Tool Test Start"], actor);
      $test_utils:assert_true(index(route, "Go west to Tool Test Start") > 0,
        "Named starting rooms resolve across areas");
      route = $llm_inspection_tools:find_route(["from_room" -> r2, "to_room" -> r1], actor);
      $test_utils:assert_true(index(route, "Go west to Tool Test Start") > 0,
        "Explicit starting objects are not replaced by the actor's location");
      details = $llm_inspection_tools:inspect_object(["object" -> r1], actor);
      $test_utils:assert_true(index(details, "Exits: east") > 0, "inspect_object should include real passage labels");
    finally
      $test_utils:destroy_if_valid(actor);
      $test_utils:destroy_if_valid(r2);
      $test_utils:destroy_if_valid(r1);
      $test_utils:destroy_if_valid(other_area);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod

endobject
