object THING [
  import_export_id -> "thing",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Thing"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property drop_denied_msg (owner: HACKER, flags: "rc") = {"You can't drop that."};
  property drop_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " dropped ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property drop_rule (owner: HACKER, flags: "rc") = 0;
  property get_denied_msg (owner: HACKER, flags: "rc") = {"You can't pick that up."};
  property get_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " picked up ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property get_rule (owner: HACKER, flags: "rc") = 0;
  property integrated_description (owner: HACKER, flags: "rc") = "";
  property is_countable_noun (owner: HACKER, flags: "rc") = true;
  property is_plural_noun (owner: HACKER, flags: "rc") = false;
  property is_proper_noun_name (owner: HACKER, flags: "rc") = false;
  property portable (owner: HACKER, flags: "rc") = true;
  property pronouns (owner: HACKER, flags: "rc") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "it/its", .ps = "it", .po = "it", .pp = "its", .pq = "its", .pr = "itself">;
  property unlocks (owner: HACKER, flags: "rc") = #-1;

  override description (owner: HACKER, flags: "rc") = "Generic thing prototype that is the basis for most items in the world.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Generic Things",
    "",
    "## Overview",
    "",
    "Things are the base class for most items in the world. They can be picked up, dropped, and moved around. Things form the foundation for more specialized items like containers and wearables.",
    "",
    "## Properties",
    "",
    "### pronouns",
    "",
    "Grammatical properties for this object (pronouns, plurality, noun type).",
    "",
    "- Default: it/its pronouns, singular, countable",
    "- Can be customized per item or use preset pronoun objects",
    "",
    "### is_plural_noun",
    "",
    "Whether this object should be treated as grammatically plural.",
    "",
    "- Default: `false`",
    "",
    "### is_countable_noun",
    "",
    "Whether this object can use articles like 'a' or 'the'.",
    "",
    "- Default: `true`",
    "",
    "### is_proper_noun_name",
    "",
    "Whether this object is a proper noun (like a name) that doesn't need articles.",
    "",
    "- Default: `false`",
    "",
    "### integrated_description",
    "",
    "Text that appears in the room description when this thing is present.",
    "",
    "- Default: empty string (not integrated)",
    "- Can be set to add flavor text about the item to the room description",
    "",
    "### get_msg and drop_msg",
    "",
    "Customizable messages shown when the item is picked up or dropped.",
    "",
    "Customize using the `@set-message` command:",
    "",
    "```",
    "@set-message sword.get_msg {nc} {grasp|grasps} {d} firmly.",
    "@set-message sword.drop_msg {nc} {release|releases} {d} carefully.",
    "```",
    "",
    "Template tokens:",
    "- `{nc}` - Actor name (capitalized)",
    "- `{grasp|grasps}` - Self-alternation verb",
    "- `{d}` - Direct object (the item)",
    "",
    "## Commands",
    "",
    "### get / take",
    "",
    "```",
    "get <item>",
    "take <item>",
    "```",
    "",
    "Picks up an item and adds it to inventory.",
    "",
    "### drop",
    "",
    "```",
    "drop <item>",
    "```",
    "",
    "Removes an item from inventory and places it in the current location.",
    "",
    "## Verbs",
    "",
    "### pronouns()",
    "",
    "Returns the pronoun set for this object.",
    "",
    "### is_plural()",
    "",
    "Returns whether this object is grammatically plural.",
    "",
    "### is_countable()",
    "",
    "Returns whether this object is countable (can use articles).",
    "",
    "### is_proper_noun()",
    "",
    "Returns whether this object is a proper noun.",
    "",
    "### integrate_description()",
    "",
    "Returns the integrated description if set, or `false` otherwise.",
    "",
    "## Example: Creating a Sword",
    "",
    "```moo",
    "sword = create($thing);",
    "sword:set_name(\"a gleaming sword\");",
    "sword.description = \"A well-crafted sword with a sharp blade.\";",
    "sword.integrated_description = \"a sword lies here, glinting in the light\";",
    "```"
  };

  method pronouns owner: ARCH_WIZARD
    "Return the validated pronoun set for this object.";
    set_task_perms(caller_perms());
    return $pronouns:validate(this.pronouns);
  endmethod

  method is_plural owner: ARCH_WIZARD
    "Returns whether this object should be treated as a plural noun.";
    "Things are singular by default but can be set to plural.";
    return this.is_plural_noun;
  endmethod

  method is_countable owner: ARCH_WIZARD
    "Returns whether this object is countable in English grammar.";
    "Things are countable by default (can use 'a' or 'the').";
    return this.is_countable_noun;
  endmethod

  method is_proper_noun owner: ARCH_WIZARD
    "Returns whether this object should be treated as a proper noun.";
    "Things are common nouns by default, not proper nouns.";
    return this.is_proper_noun_name;
  endmethod

  method integrate_description owner: ARCH_WIZARD
    "Return integrated description if set, or false. Integrated descriptions become part of the room description.";
    set_task_perms(caller_perms());
    desc = this.integrated_description;
    if (desc && desc != "")
      return desc;
    endif
    return false;
  endmethod

  method "pronoun_*" owner: ARCH_WIZARD
    "Read a pronoun form; possessive takes one selector, 'adj or 'noun.";
    set_task_perms(caller_perms());
    return $pronouns:form(this:pronouns(), tosym(verb[9..$]), @args);
  endmethod

  method can_get owner: ARCH_WIZARD
    "Check the item's get policy with the caller's read permissions.";
    set_task_perms(caller_perms());
    const {?who = player} = args;
    return $thing:_policy(this, 'get, who);
  endmethod

  method can_drop owner: ARCH_WIZARD
    "Check the item's drop policy with the caller's read permissions.";
    set_task_perms(caller_perms());
    const {?who = player} = args;
    return $thing:_policy(this, 'drop, who);
  endmethod

  method take_reachable owner: ARCH_WIZARD
    "Return whether an item is on the actor's floor or in an open nearby container.";
    "Args: {item, who}. This geometry check does not grant permission to take the item.";
    const {item, who} = args;
    valid(item) && valid(who) && valid(who.location) || return false;
    const source = item.location;
    source == who.location && return true;
    valid(source) && isa(source, $container) || return false;
    return source.open && (source.location == who || source.location == who.location);
  endmethod

  method _policy owner: ARCH_WIZARD
    "Check an item's take or drop policy with the caller's permissions.";
    set_task_perms(caller_perms());
    const {item, operation, who} = args;
    operation in {'get, 'drop} || raise(E_INVARG);
    operation == 'get && !item.portable && return false;
    if (isa(item, $wearable) && isa(item.location, $actor))
      item in item.location.wearing && return false;
    endif
    const rule = item.(tostr(operation) + "_rule");
    rule == 0 && return true;
    return $rule_engine:evaluate(rule, ['This -> item, 'Actor -> who, 'Accessor -> who])['success];
  endmethod

  method _check_transfer owner: ARCH_WIZARD
    "Check item rules and destination acceptance under the initiating principal.";
    const {item, operation, who, destination, principal} = args;
    const source = callers()[1];
    source[4] == $thing && source[2] == "transfer" || raise(E_PERM);
    set_task_perms(principal);
    $thing:_policy(item, operation, who) || return item.(tostr(operation) + "_denied_msg");
    destination:acceptable(item) || return {"That item cannot be placed there."};
    return {};
  endmethod

  method transfer owner: ARCH_WIZARD
    "Take or drop an item through one checked path. Container takes use the container operation.";
    const {item, operation, who, ?silent = false, ?reaction_context = []} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    isa(item, $thing) || raise(E_INVARG, "That object is not an item.");
    operation in {'get, 'drop} || raise(E_INVARG);
    const denied = {reason} => ['status -> 'denied, 'reason -> reason];
    const source = item.location;
    if (operation == 'get && source != who.location && isa(source, $container))
      set_task_perms(principal);
      return $container:_operate(source, 'take, who, ['item -> item], silent, reaction_context);
    endif
    const destination = operation == 'get ? who | who.location;
    valid(destination) || return denied({"There is nowhere to put that here."});
    if (operation == 'get)
      source == who.location || return denied({"That is no longer within reach."});
    else
      source == who || return denied({"You are no longer carrying that item."});
    endif
    const before = {item.owner, item.location, item.portable, item.get_rule, item.drop_rule,
      who.owner, who.location, who.wearing, destination.owner};
    const reason = $thing:_check_transfer(item, operation, who, destination, principal);
    length(reason) == 0 || return denied(reason);
    $root:check_actor(who, principal);
    equal(before, {item.owner, item.location, item.portable, item.get_rule, item.drop_rule,
      who.owner, who.location, who.wearing, destination.owner})
      || return denied({"The item or its rules changed. Try again."});
    const location = who.location;
    move(item, destination);
    item.location == destination || raise(E_INVARG, "The item did not reach its destination.");
    $reaction:_dispatch(item, tosym("on_" + tostr(operation)),
      ['Actor -> who, 'Item -> item, 'Location -> location,
        '_reaction_chain -> `reaction_context['_reaction_chain] ! E_RANGE => {}']);
    set_task_perms(principal);
    if (!silent && valid(location))
      const event = $event:mk_moved(who, @item.(tostr(operation) + "_msg"))
        :with_dobj(item):with_iobj(who);
      location:announce(event);
    endif
    return ['status -> 'applied, 'reason -> {}];
  endmethod

  method do_get owner: ARCH_WIZARD
    "Apply get through the shared transfer operation, or raise with the denial reason.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, ?silent = false} = args;
    const result = $thing:transfer(this, 'get, who, silent);
    result['status] == 'applied || raise(E_PERM, "The item cannot be moved.", result['reason]);
    return true;
  endmethod

  method do_drop owner: ARCH_WIZARD
    "Apply drop through the shared transfer operation, or raise with the denial reason.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, ?silent = false} = args;
    const result = $thing:transfer(this, 'drop, who, silent);
    result['status] == 'applied || raise(E_PERM, "The item cannot be moved.", result['reason]);
    return true;
  endmethod

  verb get (this none none) owner: ARCH_WIZARD flags: "rxd" [ ui -> ["label" -> "Take"] ]
    "Get an item through the same checks and completed events as an action.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || principal.wizard)
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    const result = $thing:transfer(this, 'get, player);
    if (result['status] == 'denied)
      player:inform_current($event:mk_error(player, @result['reason]):with_dobj(this));
    endif
    return result['status] == 'applied;
  endverb

  verb drop (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Drop an item through the same checks and completed events as an action.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || principal.wizard)
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    const result = $thing:transfer(this, 'drop, player);
    if (result['status] == 'denied)
      player:inform_current($event:mk_error(player, @result['reason]):with_dobj(this));
    endif
    return result['status] == 'applied;
  endverb

  method acceptable owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return false;
  endmethod

  method "action_get action_take" owner: ARCH_WIZARD
    "Apply get for the authenticated actor with the same policies and triggers as a command.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    return $thing:transfer(this, 'get, who, false, context)['status] == 'applied;
  endmethod

  method action_drop owner: ARCH_WIZARD
    "Apply drop for the authenticated actor with the same policies and triggers as a command.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    return $thing:transfer(this, 'drop, who, false, context)['status] == 'applied;
  endmethod

  method action_put owner: ARCH_WIZARD
    "Put this item in a container through the same checked operation as a command.";
    set_task_perms(caller_perms());
    const {who, context, destination} = args;
    return $container:_operate(destination, 'put, who, ['item -> this], false, context)['status] == 'applied;
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics - $thing defers to global get/drop topics.";
    {for_player, ?topic = ""} = args;
    "Basic get/drop covered by global help - things can override for special behavior";
    return topic == "" ? {} | 0;
  endmethod

  verb fact_unlocks (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Rule engine predicate: Key unlocks(Target)?";
    "Returns target if this object's .unlocks property matches the target.";
    "Rule engine passes (this_obj, target) as args.";
    {_, target} = args;
    unlocks = `this.unlocks ! E_PROPNF => #-1';
    if (unlocks == target)
      return target;
    endif
    return;
  endverb

  method inspection_state owner: ARCH_WIZARD
    "Describe custody and reach without running transfer policy callbacks.";
    const {?who = player} = args;
    this.location == who && return {"Carrying"};
    return $thing:take_reachable(this, who) ? {"Nearby"} | {"Out of reach"};
  endmethod

  method inspection_actions owner: ARCH_WIZARD
    "Suggest the current transfer and object commands; the parser checks policy when submitted.";
    const {?who = player} = args;
    const actions = pass(who);
    this.location != who && !$thing:take_reachable(this, who) && return {actions[1]};
    const carrying = this.location == who;
    const transfer = ["id" -> "transfer", "label" -> carrying ? "Drop" | "Take", "command" -> (carrying ? "drop " | "get ") + tostr(this), "action" -> $thing:command_action(carrying ? "drop" | "get", this:name())];
    return carrying || this.portable ? {actions[1], transfer, @actions[2..$]} | actions;
  endmethod

  method fact_is_portable owner: ARCH_WIZARD
    return this.portable;
  endmethod

  method test_grammar_and_description_defaults owner: HACKER
    "Cover default thing grammar helpers and integrated descriptions.";
    thing = this:create(true);
    thing.name = "test pebble";
    $test_utils:assert_eq(thing:pronouns(), this.pronouns, "things should inherit default pronouns");
    $test_utils:assert_false(thing:is_plural(), "things should be singular by default");
    $test_utils:assert_true(thing:is_countable(), "things should be countable by default");
    $test_utils:assert_false(thing:is_proper_noun(), "things should not be proper nouns by default");
    $test_utils:assert_eq(thing:pronoun_subject(), "it", "default subject pronoun");
    $test_utils:assert_eq(thing:pronoun_object(), "it", "default object pronoun");
    $test_utils:assert_eq(thing:pronoun_possessive('adj), "its", "default possessive adjective");
    $test_utils:assert_eq(thing:pronoun_possessive('noun), "its", "default possessive noun");
    $test_utils:assert_eq(thing:pronoun_reflexive(), "itself", "default reflexive pronoun");
    $test_utils:assert_false(thing:integrate_description(), "empty integrated description should be false");
    thing.integrated_description = "a test pebble rests here";
    $test_utils:assert_eq(thing:integrate_description(), "a test pebble rests here", "integrated description should return text");
    return true;
  endmethod

  method test_portability_and_rule_defaults owner: HACKER
    "Cover default get/drop permission helpers and portability facts.";
    thing = this:create(true);
    thing.name = "test token";
    $test_utils:assert_true(thing:can_get(player), "portable things should be gettable by default");
    $test_utils:assert_true(thing:can_drop(player), "things should be droppable by default");
    $test_utils:assert_true(thing:fact_is_portable(), "fact_is_portable should reflect default portability");
    thing.portable = false;
    $test_utils:assert_false(thing:can_get(player), "non-portable things should not be gettable");
    $test_utils:assert_false(thing:fact_is_portable(), "fact_is_portable should reflect changed portability");
    $test_utils:assert_true(thing:can_drop(player), "portable flag should not block can_drop");
    $test_utils:assert_false(thing:acceptable(thing), "plain things should not accept contents");
    return true;
  endmethod

  method test_unlock_fact_and_inspection owner: HACKER
    "Cover unlock facts and basic inspection metadata.";
    key = this:create(true);
    target = this:create(true);
    key.name = "test key";
    key.description = "A key used by the unit tests.";
    target.name = "test lock";
    key.unlocks = target;
    $test_utils:assert_eq(key:fact_unlocks(key, target), target, "fact_unlocks should return matching target");
    $test_utils:assert_eq(key:fact_unlocks(key, this), 0, "fact_unlocks should return falsey for non-matching targets");
    data = key:inspection(player);
    $test_utils:assert_eq(data["title"], "test key", "inspection should include title");
    $test_utils:assert_eq(data["description"], "A key used by the unit tests.", "inspection should include description");
    $test_utils:assert_type(data["actions"], TYPE_LIST, "inspection should include actions list");
    return true;
  endmethod
endobject
