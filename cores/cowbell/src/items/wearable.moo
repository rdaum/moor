object WEARABLE [
  import_export_id -> "wearable",
  import_export_hierarchy -> {"items"}
]
  name: "Wearable Item"
  parent: THING
  owner: HACKER
  fertile: true
  readable: true

  property body_area (owner: HACKER, flags: "rw") = false;
  property remove_msg (owner: HACKER, flags: "rw") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "remove", .for_others = "removes">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property wear_msg (owner: HACKER, flags: "rw") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "put on", .for_others = "puts on">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };

  override description (owner: HACKER, flags: "rc") = "Generic parent for items that can be worn by players.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Wearable Items",
    "",
    "## Overview",
    "",
    "Wearable items are objects that players can put on and remove from their bodies. They support body area tracking to prevent wearing conflicting items in the same location (like two hats).",
    "",
    "## Properties",
    "",
    "### body_area",
    "",
    "Optional symbol indicating which part of the body this item occupies.",
    "",
    "- Default: `false` (no body area restriction)",
    "- Examples: `'head`, `'torso`, `'feet`, `'hands`, `'neck`",
    "",
    "Set with: `@set-area <item> to <area>`",
    "",
    "When set, players cannot wear another item with the same body area simultaneously.",
    "",
    "### wear_msg and remove_msg",
    "",
    "Customizable message templates using substitutions.",
    "",
    "- `wear_msg`: Shown when item is put on (default: \"Alice puts on a hat\")",
    "- `remove_msg`: Shown when item is removed (default: \"Alice removes a hat\")",
    "",
    "Customize using the `@set-message` command:",
    "",
    "```",
    "@set-message hat.wear_msg {nc} {feel|feels} a bit more stylish putting on {d}.",
    "@set-message hat.remove_msg {nc} {feel|feels} less stylish removing {d}.",
    "```",
    "",
    "Template tokens:",
    "- `{nc}` - Actor name (capitalized)",
    "- `{feel|feels}` - Self-alternation (actor sees \"feel\", others see \"feels\")",
    "- `{d}` - Direct object (the item)",
    "",
    "## Commands",
    "",
    "### wear / put on",
    "",
    "```",
    "wear <item>",
    "put on <item>",
    "```",
    "",
    "Puts the item on the player. Returns error if:",
    "- Player doesn't have the item in inventory",
    "- Item is already being worn",
    "- Another item occupies the same body area",
    "",
    "### remove",
    "",
    "```",
    "remove <item>",
    "```",
    "",
    "Removes the item from the player. Returns error if:",
    "- Player doesn't have the item in inventory",
    "- Item is not currently being worn",
    "",
    "## Verbs for Subclasses",
    "",
    "### on_wear()",
    "",
    "Called when item is successfully worn. Override in subclasses to add custom behavior.",
    "",
    "### on_remove()",
    "",
    "Called when item is successfully removed. Override in subclasses to add custom behavior.",
    "",
    "## Example: Creating a Hat",
    "",
    "```moo",
    "hat = create($wearable);",
    "hat:set_name(\"a fancy hat\");",
    "hat.body_area = 'head;",
    "hat.description = \"A stylish hat with a wide brim.\";",
    "```"
  };

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"wear_msg", "remove_msg"})
      let bindings = $sub_utils:standard_bindings("The person wearing or removing the item.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The wearable item.",
        'example -> 'host];
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room where this happens.",
        'example -> 'location];
      return bindings;
    endif
    return pass(@args);
  endmethod

  method _conflict owner: ARCH_WIZARD
    "Read body-area conflicts without calling an overridable permission helper.";
    set_task_perms(caller_perms());
    const {item, who} = args;
    const area = item.body_area;
    !area && return #-1;
    for worn in (who.wearing)
      if (valid(worn) && worn != item && worn.body_area == area)
        return worn;
      endif
    endfor
    return #-1;
  endmethod

  method do_wear owner: ARCH_WIZARD
    "Apply wear for the authenticated wearer and emit one completed event.";
    caller == this || raise(E_PERM);
    const {who, ?silent = false, ?context = []} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    this.location == who || return false;
    !(this in who.wearing) || return false;
    valid($wearable:_conflict(this, who)) && return false;
    who.wearing = setadd(who.wearing, this);
    $reaction:_dispatch(this, 'on_wear, mapmerge(context,
      ['Actor -> who, 'Item -> this, 'Location -> who.location]));
    set_task_perms(principal);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.wear_msg):with_dobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    this:on_wear();
    return true;
  endmethod

  method do_remove owner: ARCH_WIZARD
    "Apply remove for the authenticated wearer and emit one completed event.";
    caller == this || raise(E_PERM);
    const {who, ?silent = false, ?context = []} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    this.location == who || return false;
    this in who.wearing || return false;
    who.wearing = setremove(who.wearing, this);
    $reaction:_dispatch(this, 'on_remove, mapmerge(context,
      ['Actor -> who, 'Item -> this, 'Location -> who.location]));
    set_task_perms(principal);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.remove_msg):with_dobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    this:on_remove();
    return true;
  endmethod

  verb wear (this none none) owner: ARCH_WIZARD flags: "rd"
    "Command: put on this wearable item";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || principal.wizard)
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    if (this.location != player)
      player:inform_current($event:mk_error(player, "You don't have that."));
      return;
    endif
    if (is_member(this, player.wearing))
      player:inform_current($event:mk_error(player, "You're already wearing that."));
      return;
    endif
    const conflict = this:conflicting_item();
    if (valid(conflict))
      const msg = "You're already wearing " + conflict:display_name() + " on that body part. Remove it first.";
      player:inform_current($event:mk_error(player, msg));
      return;
    endif
    this:do_wear(player);
  endverb

  verb remove (this none none) owner: ARCH_WIZARD flags: "rd"
    "Command: remove this wearable item";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || principal.wizard)
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    if (this.location != player)
      player:inform_current($event:mk_error(player, "You don't have that."));
      return;
    endif
    if (!is_member(this, player.wearing))
      player:inform_current($event:mk_error(player, "You're not wearing that."));
      return;
    endif
    this:do_remove(player);
  endverb

  method action_wear owner: ARCH_WIZARD
    "Apply wear through the same state checks and completed trigger as a command.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    $root:check_actor(who);
    return this:do_wear(who, false, context);
  endmethod

  method action_remove owner: ARCH_WIZARD
    "Apply remove through the same state checks and completed trigger as a command.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    $root:check_actor(who);
    return this:do_remove(who, false, context);
  endmethod

  method on_wear owner: HACKER
    "Called when item is worn - override in children";
    return;
  endmethod

  method on_remove owner: HACKER
    "Called when item is removed - override in children";
    return;
  endmethod

  method display_name owner: ARCH_WIZARD
    "Return display name with article for wearing context. Override for custom descriptions.";
    set_task_perms(caller_perms());
    name = this:name();
    "Check if name already has an article";
    lower_name = name:lowercase();
    if (lower_name:starts_with("the ") || lower_name:starts_with("a ") || lower_name:starts_with("an "))
      return name;
    endif
    return name:with_indefinite_article();
  endmethod

  method wearer owner: ARCH_WIZARD
    "Return who is wearing this item, or #-1 if not worn";
    set_task_perms(caller_perms());
    if (valid(this.location) && respond_to(this.location, 'is_wearing))
      if (this.location:is_wearing(this))
        return this.location;
      endif
    endif
    return #-1;
  endmethod

  method "conflicting_item conflicting_item_for" owner: ARCH_WIZARD
    "Find any item already worn at this item's body area. Returns the conflicting item or #-1 if none.";
    set_task_perms(caller_perms());
    {?who = player} = args;
    area = this.body_area;
    "If no body area defined, no conflicts possible";
    if (!area || area == false)
      return #-1;
    endif
    "Search worn items for conflicting body area";
    for worn_item in (who.wearing)
      if (valid(worn_item) && worn_item != this)
        if (worn_item.body_area == area)
          return worn_item;
        endif
      endif
    endfor
    return #-1;
  endmethod

  verb "@set-area" (this at any) owner: ARCH_WIZARD flags: "rd"
    "Set the body area for this wearable. Usage: @set-area <item> to <area>";
    if (caller != this.owner && !caller.wizard)
      player:inform_current($event:mk_error(player, "You can't do that."));
      return;
    endif
    set_task_perms(caller_perms());
    if (!iobjstr || iobjstr == "")
      player:inform_current($event:mk_error(player, "Set the area to what?"));
      return;
    endif
    area_name = iobjstr:trim():tosym();
    this.body_area = area_name;
    event = $event:mk_info(player, "Body area for ", $sub:d(), " set to ", tostr(area_name), "."):with_dobj(this);
    player:inform_current(event);
  endverb

  method moveto owner: ARCH_WIZARD
    "Prevent movement of worn items - they must be removed first";
    set_task_perms(caller_perms());
    {destination} = args;
    "Check if currently worn";
    if (valid(this:wearer()))
      raise(E_PERM, "Cannot move a worn item. Remove it first.");
    endif
    "Delegate to parent for permission checks and actual move";
    return pass(@args);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for wearable items.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("wear", "Put on an item", "Use 'wear <item>' to put on clothing, armor, or equipment.", {"don", "equip"}, 'commands, {"remove", "inventory"}), $help:mk("remove", "Take off an item", "Use 'remove <item>' to take off something you're wearing.", {"doff", "unequip"}, 'commands, {"wear", "inventory"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod

  method recycle owner: ARCH_WIZARD
    "Called by runtime before destruction. Remove from wearer's wearing list.";
    wearer = this:wearer();
    if (valid(wearer))
      wearer.wearing = setremove(wearer.wearing, this);
    endif
  endmethod
endobject
