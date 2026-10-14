object FOOD [
  import_export_id -> "food",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Food"
  parent: CONSUMABLE
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property bite_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "take", .singular_form = "takes">,
    " a bite of ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property consume_amounts (owner: ARCH_WIZARD, flags: "rc") = ["bite" -> 2, "devour" -> 99, "eat" -> 2, "nibble" -> 1];
  property devour_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "devour", .singular_form = "devours">,
    " ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property eat_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "eat", .singular_form = "eats">,
    " some of ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property munch_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "munch", .singular_form = "munches">,
    " on ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property nibble_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "nibble", .singular_form = "nibbles">,
    " at ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };

  override consume_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "eat", .singular_form = "eats">,
    " some of ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  override description (owner: ARCH_WIZARD, flags: "rc") = "Prototype for edible food items. Supports eat, bite, nibble, devour commands.";
  override finish_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "finish", .singular_form = "finishes">,
    " eating ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  override object_documentation (owner: ARCH_WIZARD, flags: "rc") = {
    "# Food",
    "",
    "## Overview",
    "",
    "Edible items that players can eat. Inherits from $consumable. Supports multiple eating verbs with different portion sizes. Food is destroyed when empty by default.",
    "",
    "## Commands",
    "",
    "- `eat <food>` - consume some of the food (default: 2 portions)",
    "- `bite <food>` - take a bite (default: 2 portions)",
    "- `nibble <food>` - take a small nibble (default: 1 portion)",
    "- `devour <food>` - consume all remaining portions (clamped to what remains)",
    "- `munch <food>` - consume a small amount (defaults to 1 portion unless configured)",
    "",
    "**Access:** You must be holding the food, or it must be in the same room.",
    "",
    "## Properties",
    "",
    "Inherited from $consumable:",
    "- `portions`: Remaining portions (default: 1)",
    "- `max_portions`: Maximum portions (default: 1)",
    "- `consume_rule`: Access control rule (0 = anyone can consume)",
    "- `on_finish`: \"destroy\" (default)",
    "",
    "Food-specific:",
    "- `consume_amounts`: Map from verb to portion amount; missing verbs default to 1.",
    "  Default: `nibble=1`, `bite=2`, `eat=2`, `devour=99` (effectively \"all remaining\").",
    "",
    "## Messages",
    "",
    "Each eating verb uses its own message property; if missing, `consume_msg` is used:",
    "- `eat_msg`, `bite_msg`, `nibble_msg`, `devour_msg`, `munch_msg`",
    "- `finish_msg`: Shown when the last portion is consumed",
    "- `empty_msg`: Shown when trying to eat an empty item",
    "",
    "**Template note:** Use `{The actor}` for actor, `{the d}` for the food item, and `{agree actor name:verb|verbs}` for proper grammar.",
    "",
    "## Rules",
    "",
    "Control who can eat with `consume_rule`:",
    "",
    "```",
    "@set-rule cake.consume_rule Accessor has(\"party invitation\")?",
    "```",
    "",
    "## Reactions",
    "",
    "Food fires these triggers:",
    "- `'on_consume`: Each eat/bite/nibble/devour/munch, bindings: `['Actor, 'Amount]`",
    "- `'on_empty`: When finished, bindings: `['Actor]`",
    "",
    "## Creating Food",
    "",
    "```moo",
    "apple = $food:create();",
    "apple:set_name_aliases(\"crisp apple\", {\"apple\"});",
    "apple.description = \"A perfectly ripe red apple.\";",
    "apple.portions = 4;",
    "apple.max_portions = 4;",
    "```"
  };

  verb "eat bite nibble devour munch" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Handle eating food - command verb for eat/bite/nibble/devour/munch.";
    "Check if player has access to the food";
    if (this.location != player && this.location != player.location)
      event = $event:mk_error(player, "You don't have ", $sub:the('dobj), "."):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if can consume";
    check = this:can_consume(player);
    if (!check['allowed])
      event = $event:mk_error(player, @check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Get amount based on verb used";
    amount = this.consume_amounts[verb] || 1;
    is_finishing = amount >= this.portions;
    if (is_finishing)
      amount = this.portions;
    endif
    "Get verb-specific message";
    msg_prop = verb + "_msg";
    msg = `this.(msg_prop) ! E_PROPNF => this.consume_msg';
    "Cache finish_msg before consumption (object may be destroyed)";
    finish_msg = this.finish_msg;
    location = player.location;
    "Announce the eating";
    if (valid(location))
      event = $event:mk_social(player, @msg):with_dobj(this):with_audience('narrative);
      location:announce(event);
    endif
    "Announce finish BEFORE consumption if finishing (object will be destroyed)";
    if (is_finishing && valid(location))
      finish_event = $event:mk_social(player, @finish_msg):with_dobj(this):with_audience('narrative);
      location:announce(finish_event);
    endif
    "Do the consumption silently (we already announced)";
    this:do_consume(player, amount, true);
  endverb

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"bite_msg", "devour_msg", "eat_msg", "munch_msg", "nibble_msg"})
      let bindings = $sub_utils:standard_bindings("The person eating.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The food.",
        'example -> 'host];
      return bindings;
    endif
    return pass(@args);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for food items.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("eat", "Eat food", "Use 'eat <food>' to eat food you're holding or that's nearby. By default it consumes 2 portions (clamped to what's left).", {"consume"}, 'commands, {"bite", "nibble"}), $help:mk("bite", "Take a bite", "Use 'bite <food>' to take a bite of food. By default it consumes 2 portions (clamped to what's left).", {}, 'commands, {"eat", "nibble"}), $help:mk("nibble", "Nibble at food", "Use 'nibble <food>' to nibble delicately at food. By default it consumes 1 portion.", {}, 'commands, {"eat", "bite"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
