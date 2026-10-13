object CONSUMABLE [
  import_export_id -> "consumable",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Consumable"
  parent: THING
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property consume_denied_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "can't", .for_others = "can't">,
    " consume ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property consume_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "consume", .for_others = "consumes">,
    " ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property consume_rule (owner: ARCH_WIZARD, flags: "rc") = 0;
  property empty_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    " is empty."
  };
  property finish_msg (owner: ARCH_WIZARD, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "finish", .for_others = "finishes">,
    " ",
    <SUB, .capitalize = false, .type = 'article_the, .binding_name = 'd, .capitalize_binding = false>,
    "."
  };
  property max_portions (owner: ARCH_WIZARD, flags: "rc") = 1;
  property on_finish (owner: ARCH_WIZARD, flags: "rc") = "destroy";
  property portions (owner: ARCH_WIZARD, flags: "rc") = 1;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Base prototype for consumable items. Provides portions, consumption rules, and triggers.";
  override object_documentation (owner: ARCH_WIZARD, flags: "rc") = {
    "# Consumables",
    "",
    "## Overview",
    "",
    "Base prototype for items that can be consumed (food, drinks, potions, etc.). Tracks portions remaining and fires triggers when consumed. Children ($food, $drink) provide the actual commands.",
    "",
    "## Properties",
    "",
    "- `portions`: Remaining portions/uses (default: 1)",
    "- `max_portions`: Maximum capacity (default: 1)",
    "- `consume_rule`: Rule for who can consume (0 = anyone)",
    "- `on_finish`: What happens when empty - `\"destroy\"` or `\"keep\"`",
    "",
    "## Messages",
    "",
    "All messages use template substitution. Customize with `@set-message`:",
    "",
    "- `consume_msg`: Shown when consuming",
    "- `consume_denied_msg`: Shown when consume_rule fails",
    "- `empty_msg`: Shown when trying to consume empty item",
    "- `finish_msg`: Shown when last portion consumed",
    "",
    "### Message Templates",
    "",
    "```",
    "@set-message potion.consume_msg {nc} {drink|drinks} {the d}, feeling magic flow through {o}.",
    "@set-message potion.finish_msg {nc} {drain|drains} the last drops from {the d}.",
    "```",
    "",
    "Available tokens: `{n}` actor, `{d}` direct object, `{the d}` with article, `{verb|verbs}` self-alt",
    "",
    "## Rules",
    "",
    "Use `@set-rule` to control who can consume:",
    "",
    "```",
    "@set-rule potion.consume_rule Accessor is_wizard?",
    "@set-rule elixir.consume_rule Accessor has(\"VIP pass\")?",
    "```",
    "",
    "Rule receives `This` (the consumable) and `Accessor` (who's trying to consume).",
    "",
    "## Triggers & Reactions",
    "",
    "Consumables fire these triggers for reactions:",
    "",
    "- `'on_consume`: Each consumption, bindings: `['Actor, 'Amount]`",
    "- `'on_empty`: When portions reach 0, bindings: `['Actor]`",
    "",
    "### Threshold Reactions",
    "",
    "React when portions cross a threshold:",
    "",
    "```",
    "@add-reaction flask.low_reaction when portions le 2 then announce \"{The t} is running low.\"",
    "```",
    "",
    "### Example: Healing Potion",
    "",
    "```",
    "@add-reaction potion.heal_reaction on on_consume then tell Actor \"You feel rejuvenated!\"",
    "```",
    "",
    "## Verbs",
    "",
    "- `can_consume(who)`: Check if who can consume, returns `['allowed, 'reason]`",
    "- `do_consume(who, amount, ?silent)`: Core consumption logic",
    "- `action_consume(who, context, ?amount)`: Reaction handler for programmatic use"
  };

  method can_consume owner: ARCH_WIZARD
    "Check if accessor can consume this. Returns ['allowed -> bool, 'reason -> msg].";
    {who} = args;
    "Check if empty";
    if (this.portions <= 0)
      return ['allowed -> false, 'reason -> this.empty_msg];
    endif
    "No rule = anyone can consume";
    if (this.consume_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate consume rule";
    result = $rule_engine:evaluate(this.consume_rule, ['This -> this, 'Accessor -> who]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      return ['allowed -> false, 'reason -> this.consume_denied_msg];
    endif
  endmethod

  method do_consume owner: ARCH_WIZARD
    "Core consumption logic. Decrements portions, announces, fires triggers.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_consume must be called by this object");
    {who, amount, ?silent = false} = args;
    const principal = caller_perms();
    principal == who || principal == who.owner || principal.wizard || raise(E_PERM);
    "Clamp amount to available portions";
    amount = min(amount, this.portions);
    amount <= 0 && return false;
    "Decrement portions";
    $reaction:change(this, 'portions, 'decrement, amount, ['Actor -> who, 'Amount -> amount]);
    "Fire on_consume trigger";
    this:fire_trigger('on_consume, ['Actor -> who, 'Amount -> amount]);
    "Announce consumption";
    if (!silent && valid(who.location))
      msg = this.consume_msg;
      event = $event:mk_social(who, @msg):with_dobj(this):with_audience('narrative);
      who.location:announce(event);
    endif
    "Check if finished";
    if (this.portions <= 0)
      "Fire on_empty trigger";
      this:fire_trigger('on_empty, ['Actor -> who]);
      "Announce finish";
      if (!silent && valid(who.location))
        finish_event = $event:mk_social(who, @this.finish_msg):with_dobj(this):with_audience('narrative);
        who.location:announce(finish_event);
      endif
      "Handle on_finish behavior";
      if (this.on_finish == "destroy")
        this:destroy();
      endif
    endif
    return true;
  endmethod

  method action_consume owner: ARCH_WIZARD
    "Action handler for reactions: make actor consume this item.";
    const principal = caller_perms();
    {who, context, ?amount = 1} = args;
    principal == who || principal == who.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    "Must be holding the item or it must be in same room";
    if (this.location != who && this.location != who.location)
      return false;
    endif
    "Check if allowed";
    check = this:can_consume(who);
    !check['allowed] && return false;
    "Do the consumption";
    return this:do_consume(who, amount);
  endmethod

  method consume_for_test owner: ARCH_WIZARD
    "Test helper for the owner or a wizard.";
    caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    {who, amount} = args;
    "Check first";
    check = this:can_consume(who);
    if (!check['allowed])
      return ["allowed" -> false, "reason" -> check['reason]];
    endif
    "Manually do what do_consume does, without caller check";
    old_portions = this.portions;
    $reaction:change(this, 'portions, 'set, max(0, this.portions - amount), ['Actor -> who, 'Amount -> amount]);
    this:fire_trigger('on_consume, ['Actor -> who, 'Amount -> amount]);
    if (this.portions <= 0)
      this:fire_trigger('on_empty, ['Actor -> who]);
    endif
    return ["allowed" -> true, "old_portions" -> old_portions, "new_portions" -> this.portions];
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for consumable items.";
    {for_player, ?topic = ""} = args;
    "Base consumable - no specific commands, children define them";
    topic == "" && return {};
    return 0;
  endmethod
endobject
