object DRINK [
  import_export_id -> "drink",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Drink"
  parent: CONSUMABLE
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property consume_amounts (owner: ARCH_WIZARD, flags: "rc") = ["drink" -> 2, "gulp" -> 3, "quaff" -> 5, "sip" -> 1];
  property contents_name (owner: ARCH_WIZARD, flags: "rc") = "liquid";
  property drink_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "drink", .singular_form = "drinks">,
    " from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property empty_name (owner: ARCH_WIZARD, flags: "rc") = "";
  property fill_time (owner: ARCH_WIZARD, flags: "rc") = 0;
  property gulp_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "gulp", .singular_form = "gulps">,
    " from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property quaff_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "quaff", .singular_form = "quaffs">,
    " deeply from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property refill_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "refill", .singular_form = "refills">,
    " ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property refillable (owner: ARCH_WIZARD, flags: "rc") = true;
  property sip_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "take", .singular_form = "takes">,
    " a small sip from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property temp_descriptions (owner: ARCH_WIZARD, flags: "rc") = [
    "cold" -> "cold",
    "hot" -> "steaming",
    "icy" -> "ice-cold",
    "room" -> "",
    "warm" -> "warm"
  ];
  property temperature (owner: ARCH_WIZARD, flags: "rc") = "room";

  override consume_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "drink", .singular_form = "drinks">,
    " from ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  override description (owner: ARCH_WIZARD, flags: "rc") = "Prototype for drinkable beverages in vessels. Supports sip, drink, gulp, quaff and refill.";
  override finish_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'article_the, .binding_name = 'actor>,
    " ",
    <SUB, .type = 'agree, .binding_name = 'actor, .subject_form = 'name, .plural_form = "drain", .singular_form = "drains">,
    " the last of ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  override max_portions (owner: ARCH_WIZARD, flags: "rc") = 10;
  override object_documentation (owner: ARCH_WIZARD, flags: "rc") = {
    "# Drink",
    "",
    "## Overview",
    "",
    "Beverage vessels that players can drink from. Inherits from $consumable. The vessel persists when empty by default and can be refilled. Supports multiple drinking verbs with different portion sizes.",
    "",
    "## Commands",
    "",
    "- `drink <beverage>` - take a normal drink (default: 2 portions)",
    "- `sip <beverage>` - take a small sip (default: 1 portion)",
    "- `gulp <beverage>` / `quaff <beverage>` - drink quickly (defaults: 3 / 5 portions)",
    "- `refill <vessel>` - refill to full capacity (must be holding the vessel)",
    "",
    "**Access:** To drink, you must be holding the vessel or it must be in the same room. To refill, you must be holding it.",
    "",
    "## Properties",
    "",
    "Inherited from $consumable:",
    "- `portions`: Remaining portions (default: 10)",
    "- `max_portions`: Vessel capacity (default: 10)",
    "- `consume_rule`: Access control rule (0 = anyone can consume)",
    "- `on_finish`: \"keep\" (default)",
    "",
    "Drink-specific:",
    "- `consume_amounts`: Map from verb to portion amount; missing verbs default to 1.",
    "  Default: `sip=1`, `drink=2`, `gulp=3`, `quaff=5`.",
    "- `refillable`: Whether `refill` is allowed (default: true)",
    "- `fill_time`: Last refill time (set to `time()` by `refill`; default: 0)",
    "- `temperature`: String label for temperature (default: \"room\")",
    "- `temp_descriptions`: Map of temperature labels to adjectives",
    "- `contents_name`: Name of the contents (default: \"liquid\")",
    "- `empty_name`: Optional alternate name when empty (default: \"\")",
    "",
    "## Messages",
    "",
    "Each drinking verb uses its own message property; if missing, `consume_msg` is used:",
    "- `drink_msg`, `sip_msg`, `gulp_msg`, `quaff_msg`",
    "- `refill_msg`: Shown when refilling",
    "- `finish_msg`: Shown when draining the last portion",
    "- `empty_msg`: Shown when trying to drink an empty vessel",
    "",
    "**Template note:** Use `{The actor}` for actor, `{the d}` for the vessel, and `{agree actor name:verb|verbs}` for proper grammar.",
    "",
    "## Rules",
    "",
    "Control who can drink with `consume_rule`:",
    "",
    "```",
    "@set-rule chalice.consume_rule This owner_is(Accessor)?",
    "```",
    "",
    "## Reactions",
    "",
    "Drinks fire these triggers:",
    "- `'on_consume`: Each sip/drink/gulp/quaff, bindings: `['Actor, 'Amount]`",
    "- `'on_empty`: When drained, bindings: `['Actor]`",
    "- `'on_refill`: When refilled, bindings: `['Actor]`",
    "",
    "## Creating Drinks",
    "",
    "```moo",
    "mug = $drink:create();",
    "mug:set_name_aliases(\"steaming mug of coffee\", {\"mug\", \"coffee\"});",
    "mug.description = \"Dark roast, still hot.\";",
    "mug.portions = 10;",
    "mug.max_portions = 10;",
    "mug.contents_name = \"coffee\";",
    "```"
  };
  override on_finish (owner: ARCH_WIZARD, flags: "rc") = "keep";
  override portions (owner: ARCH_WIZARD, flags: "rc") = 10;

  verb "drink sip gulp quaff" (this none none) owner: ARCH_WIZARD flags: "rd"
    "Handle drinking - command verb for drink/sip/gulp/quaff.";
    "Check if player has access to the drink";
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
    "Cache finish_msg before consumption";
    finish_msg = this.finish_msg;
    location = player.location;
    "Announce the drinking";
    if (valid(location))
      event = $event:mk_social(player, @msg):with_dobj(this):with_audience('narrative);
      location:announce(event);
    endif
    "Announce finish BEFORE consumption if finishing";
    if (is_finishing && valid(location))
      finish_event = $event:mk_social(player, @finish_msg):with_dobj(this):with_audience('narrative);
      location:announce(finish_event);
    endif
    "Do the consumption silently";
    this:do_consume(player, amount, true);
    "Tell player remaining amount if not finished";
    if (!is_finishing)
      player:inform_current($event:mk_info(player, "(" + tostr(this.portions) + " remaining)"));
    endif
  endverb

  verb refill (this none none) owner: ARCH_WIZARD flags: "rd"
    "Handle refilling the drink vessel.";
    "Check if player has the drink";
    if (this.location != player)
      event = $event:mk_error(player, "You need to be holding ", $sub:the('dobj), " to refill it."):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if refillable";
    if (!this.refillable)
      event = $event:mk_error(player, $sub:thec('dobj), " can't be refilled."):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if already full";
    if (this.portions >= this.max_portions)
      event = $event:mk_error(player, $sub:thec('dobj), " ", $sub:agree('dobj, 'name, "are", "is"), " already full."):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Refill to max";
    this.portions = this.max_portions;
    this.fill_time = time();
    "Announce";
    if (valid(player.location))
      event = $event:mk_social(player, @this.refill_msg):with_dobj(this):with_audience('narrative);
      player.location:announce(event);
    endif
    "Fire trigger";
    this:fire_trigger('on_refill, ['Actor -> player]);
  endverb

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"drink_msg", "gulp_msg", "quaff_msg", "refill_msg", "sip_msg"})
      let bindings = $sub_utils:standard_bindings("The person drinking or refilling.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The drink.",
        'example -> 'host];
      return bindings;
    endif
    return pass(@args);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for drink items.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("drink", "Drink a beverage", "Use 'drink <beverage>' to drink from a vessel you're holding or that's nearby. By default it consumes 2 portions (clamped to what's left).", {}, 'commands, {"sip", "gulp"}), $help:mk("sip", "Sip a drink", "Use 'sip <beverage>' to take a small, delicate sip from a drink. By default it consumes 1 portion.", {}, 'commands, {"drink", "gulp"}), $help:mk("gulp", "Gulp down a drink", "Use 'gulp <beverage>' to gulp down a larger portion of a drink quickly. By default it consumes 3 portions (clamped to what's left).", {"quaff"}, 'commands, {"drink", "sip"}), $help:mk("refill", "Refill a vessel", "Use 'refill <vessel>' to refill a drink vessel you're holding back to full capacity.", {}, 'commands, {"drink"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
