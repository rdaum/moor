object CONTAINER [
  import_export_id -> "container",
  import_export_hierarchy -> {"items"}
]
  name: "Generic Container"
  parent: THING
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property already_closed_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'dobj>, " is already closed."};
  property already_locked_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'dobj>, " is already locked."};
  property already_open_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'dobj>, " is already open."};
  property already_unlocked_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'dobj>, " is already unlocked."};
  property close_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " won't close."};
  property close_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "close", .for_others = "closes">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property close_rule (owner: HACKER, flags: "rc") = 0;
  property lock_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " won't lock."};
  property lock_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "lock", .for_others = "locks">,
    " ",
    <SUB, .capitalize = false, .type = 'iobj>,
    " with ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property lock_rule (owner: HACKER, flags: "rc") = 0;
  property locked (owner: HACKER, flags: "r") = false;
  property not_closeable_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " can't be closed."};
  property not_lockable_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " doesn't have a lock."};
  property not_openable_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " can't be opened."};
  property not_unlockable_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " doesn't have a lock."};
  property open (owner: HACKER, flags: "r") = true;
  property open_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " won't open."};
  property open_locked_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'dobj>, " is locked."};
  property open_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "open", .for_others = "opens">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property open_rule (owner: HACKER, flags: "rc") = 0;
  property put_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " is closed."};
  property put_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .capitalize = false, .type = 'self_alt, .for_self = "put", .for_others = "puts">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    " ",
    <SUB, .capitalize = false, .type = 'binding, .binding_name = 'prep>,
    " ",
    <SUB, .capitalize = false, .type = 'iobj>,
    "."
  };
  property put_prep_display (owner: HACKER, flags: "rc") = "in";
  property put_preps (owner: HACKER, flags: "rc") = {"in", "inside", "into"};
  property put_rule (owner: HACKER, flags: "rc") = 0;
  property take_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " is closed."};
  property take_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "take", .for_others = "takes">,
    " ",
    <SUB, .capitalize = false, .type = 'dobj>,
    " from ",
    <SUB, .capitalize = false, .type = 'iobj>,
    "."
  };
  property take_rule (owner: HACKER, flags: "rc") = 0;
  property unlock_denied_msg (owner: HACKER, flags: "rc") = {<SUB, .capitalize = true, .type = 'iobj>, " won't unlock."};
  property unlock_msg (owner: HACKER, flags: "rc") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " ",
    <SUB, .type = 'self_alt, .for_self = "unlock", .for_others = "unlocks">,
    " ",
    <SUB, .capitalize = false, .type = 'iobj>,
    " with ",
    <SUB, .capitalize = false, .type = 'dobj>,
    "."
  };
  property unlock_rule (owner: HACKER, flags: "rc") = 0;

  override description (owner: HACKER, flags: "rc") = "Generic container prototype for objects that can hold other items.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Containers",
    "",
    "## Overview",
    "",
    "Containers are objects that can hold other items. They inherit from Thing and add the ability to put items inside and take items out. Containers support open/close functionality and rule-based access control for all operations.",
    "",
    "## Properties",
    "",
    "### take_msg and put_msg",
    "",
    "Customizable messages shown when items are taken from or put into the container.",
    "",
    "- `take_msg`: Shown when removing an item (default: \"Alice takes a sword from the chest.\")",
    "- `put_msg`: Shown when inserting an item (default: \"Alice puts a sword in the chest.\")",
    "",
    "Customize using the `@set-message` command:",
    "",
    "```",
    "@set-message chest.take_msg {nc} {retrieve|retrieves} {d} from {i}.",
    "@set-message chest.put_msg {nc} {store|stores} {d} in {i}.",
    "```",
    "",
    "Template tokens:",
    "- `{nc}` - Actor name (capitalized)",
    "- `{retrieve|retrieves}` - Self-alternation verb",
    "- `{d}` - Direct object (the item)",
    "- `{i}` - Indirect object (the container)",
    "",
    "### open property",
    "",
    "Boolean indicating whether the container is open or closed. Containers must be open to take or put items.",
    "",
    "- `open`: Current open state (default: `true`)",
    "- `open_msg`: Message shown when opening (template)",
    "- `close_msg`: Message shown when closing (template)",
    "- `already_open_msg`: Message shown when trying to open an already open container",
    "- `already_closed_msg`: Message shown when trying to close an already closed container",
    "- `not_openable_msg`: Message shown when trying to open a container with no open_rule",
    "- `not_closeable_msg`: Message shown when trying to close a container with no close_rule",
    "",
    "### locked property",
    "",
    "Boolean indicating whether the container is locked. Locked containers cannot be opened until unlocked.",
    "",
    "- `locked`: Current lock state (default: `false`)",
    "- `lock_msg`: Message shown when locking (template)",
    "- `unlock_msg`: Message shown when unlocking (template)",
    "- `not_lockable_msg`: Message shown when trying to lock a container with no lock_rule",
    "- `not_unlockable_msg`: Message shown when trying to unlock a container with no unlock_rule",
    "",
    "### Access Control Rules",
    "",
    "Rule-based access control for container operations. Rules are datalog-style expressions that determine who can perform actions.",
    "",
    "- `take_rule`: Controls who can remove items (default: `0` = public access)",
    "- `put_rule`: Controls who can insert items (default: `0` = public access)",
    "- `open_rule`: Controls who can open the container (default: `0` = public access)",
    "- `close_rule`: Controls who can close the container (default: `0` = public access)",
    "- `lock_rule`: Controls who can lock the container (default: `0` = not lockable)",
    "- `unlock_rule`: Controls who can unlock the container (default: `0` = not unlockable)",
    "- `take_denied_msg`: Message shown when take is denied (template)",
    "- `put_denied_msg`: Message shown when put is denied (template)",
    "- `open_denied_msg`: Message shown when open is denied (template)",
    "- `close_denied_msg`: Message shown when close is denied (template)",
    "- `lock_denied_msg`: Message shown when lock is denied (template)",
    "- `unlock_denied_msg`: Message shown when unlock is denied (template)",
    "",
    "Set rules using the `@set-rule` command:",
    "",
    "```",
    "@set-rule chest.take_rule This owner_is(Accessor)?",
    "@set-rule chest.open_rule Accessor is_wizard?",
    "```",
    "",
    "Clear rules to restore public access:",
    "",
    "```",
    "@clear-rule chest.take_rule",
    "```",
    "",
    "View current rules:",
    "",
    "```",
    "@show-rule chest.take_rule",
    "```",
    "",
    "**Rule Variables** (must be capitalized):",
    "- `This` - The container itself",
    "- `Accessor` - The player attempting the action",
    "- `Dobj` - The item being taken/put (for take_rule/put_rule)",
    "- `Key` - The key object being used (for lock_rule/unlock_rule)",
    "",
    "**Available Fact Predicates** (from $root):",
    "- `This owner_is(Accessor)?` - Check if accessor owns this container",
    "- `This contains(Dobj)?` - Check if container holds item",
    "- `Dobj owner_is(Accessor)?` - Check if accessor owns the item",
    "- `Dobj location_is(This)?` - Check if item is inside this container",
    "",
    "**Available Fact Predicates** (from $container):",
    "- `This is_locked?` - Check if container is locked",
    "- `This is_open?` - Check if container is open",
    "",
    "**Available Fact Predicates** (from $actor):",
    "- `Accessor is_wizard?` - Check if accessor is a wizard",
    "- `Accessor is_programmer?` - Check if accessor is a programmer",
    "- `Accessor is_builder?` - Check if accessor is a builder",
    "",
    "**Rule Operators**:",
    "- `AND` - Both conditions must be true",
    "- `OR` - Either condition must be true",
    "- `NOT` - Condition must be false (bounded negation only)",
    "",
    "## Commands",
    "",
    "### get / take / steal <item> from <container>",
    "",
    "```",
    "get sword from chest",
    "take sword from chest",
    "steal sword from chest",
    "```",
    "",
    "Removes an item from the container and adds it to inventory. The container must be open.",
    "",
    "### put <item> in <container>",
    "",
    "```",
    "put sword in chest",
    "```",
    "",
    "Places an item from inventory into the container. The container must be open.",
    "",
    "### open <container>",
    "",
    "```",
    "open chest",
    "```",
    "",
    "Opens the container. The container must not be locked. Access is controlled by `open_rule`.",
    "",
    "### close <container>",
    "",
    "```",
    "close chest",
    "```",
    "",
    "Closes the container. Access is controlled by `close_rule`.",
    "",
    "### lock <container> with <key>",
    "",
    "```",
    "lock chest with brass key",
    "```",
    "",
    "Locks the container using a key object. Access is controlled by `lock_rule`.",
    "",
    "### unlock <container> with <key>",
    "",
    "```",
    "unlock chest with brass key",
    "```",
    "",
    "Unlocks the container using a key object. Access is controlled by `unlock_rule`.",
    "",
    "## Behavior",
    "",
    "- Containers must be open to take or put items",
    "- Locked containers cannot be opened until unlocked",
    "- Containers accept items by default (override `acceptable()` to restrict)",
    "- Looking at a container shows its contents",
    "- Items can be taken from containers if the player can carry them",
    "- Items are moved into the container, not copied",
    "",
    "## Example: Creating a Lockable Treasure Chest",
    "",
    "```moo",
    "chest = create($container);",
    "chest:set_name(\"a wooden chest\");",
    "chest.description = \"An old wooden chest with brass hinges.\";",
    "@set-rule chest.lock_rule This owner_is(Accessor)?",
    "@set-rule chest.unlock_rule This owner_is(Accessor)?",
    "sword = create($thing);",
    "sword:set_name(\"a gleaming sword\");",
    "sword:moveto(chest);",
    "```",
    "",
    "Example workflow:",
    "",
    "```",
    "> close chest",
    "You close the wooden chest.",
    "> lock chest with key",
    "You lock the wooden chest with the brass key.",
    "> unlock chest with key",
    "You unlock the wooden chest with the brass key.",
    "> open chest",
    "You open the wooden chest.",
    "> get sword from chest",
    "You take the gleaming sword from the wooden chest.",
    "```"
  };

  method reference_kind owner: HACKER
    "Identify containers in client references and choices.";
    return "container";
  endmethod

  method acceptable owner: HACKER
    "Containers accept items by default";
    return true;
  endmethod

  method fact_is_locked owner: HACKER
    "Rule predicate: Is this container locked?";
    {container} = args;
    return container.locked;
  endmethod

  method can_lock owner: HACKER
    "Check if accessor can lock this container with key. Returns {allowed, reason}.";
    {accessor, key} = args;
    "No rule = public access";
    if (this.lock_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate lock rule";
    result = $rule_engine:evaluate(this.lock_rule, ['This -> this, 'Accessor -> accessor, 'Key -> key]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.lock_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  method _log owner: ARCH_WIZARD
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this, {{"builtin_call", "server_log"}});
    server_log(@args);
  endmethod

  method can_unlock owner: HACKER
    "Check if accessor can unlock this container with key. Returns {allowed, reason}.";
    {accessor, key} = args;
    "No rule = public access";
    if (this.unlock_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate unlock rule";
    result = $rule_engine:evaluate(this.unlock_rule, ['This -> this, 'Accessor -> accessor, 'Key -> key]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.unlock_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  method can_take_from owner: HACKER
    "Check if accessor can take dobj from this container. Returns {allowed, reason}.";
    {accessor, dobj} = args;
    "Check if container is open";
    if (!this.open)
      return ['allowed -> false, 'reason -> this.take_denied_msg];
    endif
    "No rule = public access";
    if (this.take_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate take rule";
    result = $rule_engine:evaluate(this.take_rule, ['This -> this, 'Accessor -> accessor, 'Dobj -> dobj]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.take_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  method can_put_into owner: HACKER
    "Check if accessor can put dobj into this container. Returns {allowed, reason}.";
    {accessor, dobj} = args;
    "Check if container is open";
    if (!this.open)
      return ['allowed -> false, 'reason -> this.put_denied_msg];
    endif
    "No rule = public access";
    if (this.put_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate put rule";
    result = $rule_engine:evaluate(this.put_rule, ['This -> this, 'Accessor -> accessor, 'Dobj -> dobj]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.put_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  method can_view_contents owner: ARCH_WIZARD
    "Apply the container's viewing rule and recheck reach/open state after authored callbacks.";
    set_task_perms(caller_perms());
    const {viewer} = args;
    const reachable = this.location == viewer || $thing:take_reachable(this, viewer);
    this.open && reachable || return false;
    if (this.take_rule != 0)
      const result = $rule_engine:evaluate(this.take_rule, ['This -> this, 'Accessor -> viewer]);
      result['success] || return false;
    endif
    return this.open && (this.location == viewer || $thing:take_reachable(this, viewer));
  endmethod

  method contents owner: ARCH_WIZARD
    "Expose only contents the current viewer can see, for looks, examination, and suggestions.";
    set_task_perms(caller_perms());
    this:can_view_contents(player) || return {};
    return pass();
  endmethod

  method match_scope_for owner: ARCH_WIZARD
    "Contribute visible contents to an explicitly targeted environmental search.";
    set_task_perms(caller_perms());
    const {actor, ?context = []} = args;
    actor == player || raise(E_PERM);
    return this:contents();
  endmethod

  method look_self owner: ARCH_WIZARD
    "Capture the description and visible contents, resolving a denied view with its container binding.";
    set_task_perms(caller_perms());
    const description = this:description();
    let blocks = $format:blocks(typeof(description) == TYPE_LIST ? description | {description});
    if (this:can_view_contents(player))
      const contents = {item for item in (this.contents) if (valid(item))};
      if (length(contents))
        let parts = {"It contains "};
        for i in [1..length(contents)]
          if (i > 1)
            parts = {@parts, i == length(contents) ? " and " | ", "};
          endif
          parts = {@parts, $format.annotation:object(contents[i], contents[i]:display_name())};
        endfor
        blocks = {@blocks, $format.paragraph:mk(@parts, ".")};
      endif
    else
      const denied = $event:mk_info(player, this.take_denied_msg):with_iobj(this):resolve(player);
      blocks = {@blocks, @flycontents(denied)};
    endif
    return <$look, .what = this, .title = this:name(), .description = $document_template:mk('blocks, blocks)>;
  endmethod

  method inspection_state owner: ARCH_WIZARD
    "Describe the container's observable custody, opening, and lock state.";
    const {?who = player} = args;
    const state = pass(who);
    this.location != who && !$thing:take_reachable(this, who) && return state;
    return this.locked ? {@state, "Closed", "Locked"} | {@state, this.open ? "Open" | "Closed"};
  endmethod

  method inspection_commands owner: ARCH_WIZARD
    "Choose commands for the container's current state without executing its policies.";
    const {?who = player} = args;
    let actions = {};
    for action in (pass(who))
      const id = action["id"];
      if (id == "open" && (this.open || this.locked) || (id == "close" && !this.open) || (id == "lock" && (this.open || this.locked)) || (id == "unlock" && !this.locked))
        continue;
      endif
      actions = {@actions, action};
    endfor
    if (this.open)
      const ref = tostr(this);
      actions = {@actions, ["id" -> "put", "label" -> "Put inside", "command" -> "put {input} in " + ref, "action" -> $container:command_action("put", this:name()), "input" -> ["label" -> "What are you putting inside?", "placeholder" -> "Find a carried item\u2026", "suggestions" -> ["provider" -> $url_utils:to_curie_str(this), "source" -> "inventory"]]], ["id" -> "take_from", "label" -> "Take from", "command" -> "get {input} from " + ref, "action" -> $container:command_action("get", this:name()), "input" -> ["label" -> "What are you taking out?", "placeholder" -> "Find an item inside\u2026", "suggestions" -> ["provider" -> $url_utils:to_curie_str(this), "source" -> "contents"]]]};
    endif
    return actions;
  endmethod

  verb "get take steal grab" (any from this) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["arguments" -> ["dobj" -> "Item"], "label" -> "Take", "prefix" -> "Take from"]
  ]
    "Take an object from this container";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    if (!dobjstr || dobjstr == "")
      event = $event:mk_error(player, "Take what?");
      player:inform_current(event);
      return;
    endif
    "Match the object from dobjstr - search in this container's contents";
    try
      dobj = $match:match_object(dobjstr, this);
    except e (ANY)
      event = $event:mk_error(player, $sub:i(), " doesn't have that."):with_iobj(this);
      player:inform_current(event);
      return;
    endtry
    if (!valid(dobj) || typeof(dobj) != TYPE_OBJ)
      event = $event:mk_error(player, $sub:i(), " doesn't have that."):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    if (dobj.location != this)
      event = $event:mk_error(player, $sub:d(), " isn't in ", $sub:i(), "."):with_dobj(dobj):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    const access_check = this:can_take_from(player, dobj);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(dobj):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if player can accept the item";
    if (!player:acceptable(dobj))
      event = $event:mk_error(player, "You can't carry ", $sub:d(), "."):with_dobj(dobj);
      player:inform_current(event);
      return;
    endif
    try
      this:do_take_from(player, dobj);
    except e (E_PERM)
      const msg = length(e) > 2 ? e[2] | "You can't take that from " + this:name() + ".";
      event = $event:mk_error(player, msg):with_dobj(dobj);
      player:inform_current(event);
    endtry
  endverb

  verb put (any any this) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["arguments" -> ["dobj" -> "Item"], "label" -> "Put inside"]
  ]
    "Put an object in/on this container.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    "Check preposition is valid for this container.";
    if (!(prepstr in this.put_preps))
      event = $event:mk_error(player, "You can't put things ", prepstr, " ", $sub:i(), "."):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    if (!dobjstr || dobjstr == "")
      event = $event:mk_error(player, "Put what ", prepstr, " ", $sub:i(), "?"):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Match the object being put from player's perspective.";
    try
      dobj = $match:match_object(dobjstr, player);
    except e (ANY)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endtry
    if (!valid(dobj) || typeof(dobj) != TYPE_OBJ)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endif
    if (dobj.location != player)
      event = $event:mk_error(player, "You don't have ", $sub:d(), "."):with_dobj(dobj);
      player:inform_current(event);
      return;
    endif
    if (dobj == this)
      event = $event:mk_error(player, "You can't put ", $sub:d(), " inside itself."):with_dobj(dobj);
      player:inform_current(event);
      return;
    endif
    "Check access via rule.";
    const access_check = this:can_put_into(player, dobj);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(dobj):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if this container can accept the item.";
    if (!this:acceptable(dobj))
      event = $event:mk_error(player, $sub:i(), " can't hold ", $sub:d(), "."):with_dobj(dobj):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    try
      this:do_put_into(player, dobj, prepstr);
    except e (E_PERM)
      const msg = length(e) > 2 ? e[2] | "You can't put that " + prepstr + " " + this:name() + ".";
      event = $event:mk_error(player, msg):with_dobj(dobj);
      player:inform_current(event);
    endtry
  endverb

  verb lock (this with any) owner: ARCH_WIZARD flags: "rxd"
    "Lock this container with a key";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    "Check if container is lockable";
    if (this.lock_rule == 0)
      event = $event:mk_error(player, @this.not_lockable_msg):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    if (!iobjstr || iobjstr == "")
      event = $event:mk_error(player, "Lock ", $sub:i(), " with what?"):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Match the key object from player's perspective";
    let key = #-1;
    try
      key = $match:match_object(iobjstr, player);
    except e (ANY)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endtry
    if (!valid(key) || typeof(key) != TYPE_OBJ)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endif
    "Check if already locked";
    if (this.locked)
      event = $event:mk_error(player, $sub:i(), " is already locked."):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    const access_check = this:can_lock(player, key);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(key):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_lock(player, key);
  endverb

  verb unlock (this with any) owner: ARCH_WIZARD flags: "rxd"
    "Unlock this container with a key";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    "Check if container is unlockable";
    if (this.unlock_rule == 0)
      event = $event:mk_error(player, @this.not_unlockable_msg):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    if (!iobjstr || iobjstr == "")
      event = $event:mk_error(player, "Unlock ", $sub:i(), " with what?"):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Match the key object from player's perspective";
    let key = #-1;
    try
      key = $match:match_object(iobjstr, player);
    except e (ANY)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endtry
    if (!valid(key) || typeof(key) != TYPE_OBJ)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endif
    "Check if already unlocked";
    if (!this.locked)
      event = $event:mk_error(player, $sub:i(), " is already unlocked."):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    const access_check = this:can_unlock(player, key);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(key):with_iobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_unlock(player, key);
  endverb

  method can_open owner: HACKER
    "Check if accessor can open this container. Returns {allowed, reason}.";
    {accessor} = args;
    "No rule = public access";
    if (this.open_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate open rule";
    result = $rule_engine:evaluate(this.open_rule, ['This -> this, 'Accessor -> accessor]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.open_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  method can_close owner: HACKER
    "Check if accessor can close this container. Returns {allowed, reason}.";
    {accessor} = args;
    "No rule = public access";
    if (this.close_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate close rule";
    result = $rule_engine:evaluate(this.close_rule, ['This -> this, 'Accessor -> accessor]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      reason = this.close_denied_msg;
      return ['allowed -> false, 'reason -> reason];
    endif
  endmethod

  verb "open op*" (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Open this container";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    "Check if already open";
    if (this.open)
      event = $event:mk_error(player, @this.already_open_msg):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check if container is locked";
    if (this.locked)
      event = $event:mk_error(player, @this.open_locked_msg):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    const access_check = this:can_open(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_open(player);
  endverb

  verb close (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Close this container";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    let event = false;
    set_task_perms(actor == #-1 ? player | actor);
    "Check if already closed";
    if (!this.open)
      event = $event:mk_error(player, @this.already_closed_msg):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    const access_check = this:can_close(player);
    if (!access_check['allowed])
      event = $event:mk_error(player, @access_check['reason]):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    this:do_close(player);
  endverb

  method fact_is_open owner: HACKER
    "Rule predicate: Is this container open?";
    return this.open;
  endmethod

  method do_take_from owner: ARCH_WIZARD
    "Core: move item from container to actor's inventory.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_take_from must be called by this object");
    const {who, item, ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    valid(item) && item.location == this && this.open || return false;
    const policy = this.take_rule;
    this:can_take_from(who, item)['allowed] && who:acceptable(item) || return false;
    item.location == this && this.open && this.take_rule == policy || return false;
    const moved = item:moveto(who);
    typeof(moved) == TYPE_ERR && raise(moved);
    item.location == who || raise(E_INVARG, "Item movement did not reach its destination");
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.take_msg):with_dobj(item):with_iobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    this:fire_trigger('on_take, ['Actor -> who, 'Item -> item]);
    return true;
  endmethod

  method do_put_into owner: ARCH_WIZARD
    "Core: move item from actor into this container.";
    "Only callable by this object itself.";
    caller != this && raise(E_PERM, "do_put_into must be called by this object");
    const {who, item, ?prep = "", ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    valid(item) && item.location == who && this.open || return false;
    const policy = this.put_rule;
    this:can_put_into(who, item)['allowed] && this:acceptable(item) || return false;
    item.location == who && this.open && this.put_rule == policy || return false;
    const moved = item:moveto(this);
    typeof(moved) == TYPE_ERR && raise(moved);
    item.location == this || raise(E_INVARG, "Item movement did not reach its destination");
    this:fire_trigger('on_put, ['Actor -> who, 'Item -> item]);
    if (!silent && valid(who.location))
      "Use prep passed in, or fall back to put_prep_display property.";
      const display_prep = prep != "" ? prep | this.put_prep_display;
      const msg = $sub_utils:compile("{Nc} {put|puts} {d} " + display_prep + " {i}.");
      const event = $event:mk_info(who, @msg):with_dobj(item):with_iobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    return true;
  endmethod

  method do_open owner: ARCH_WIZARD
    "Core: open this container.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_open must be called by this object");
    const {who, ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    !this.open && !this.locked || return false;
    const policy = this.open_rule;
    this:can_open(who)['allowed] || return false;
    !this.open && !this.locked && this.open_rule == policy || return false;
    this.open = true;
    this:fire_trigger('on_open, ['Actor -> who]);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.open_msg):with_dobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    return true;
  endmethod

  method do_close owner: ARCH_WIZARD
    "Core: close this container.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_close must be called by this object");
    const {who, ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    this.open || return false;
    const policy = this.close_rule;
    this:can_close(who)['allowed] || return false;
    this.open && this.close_rule == policy || return false;
    this.open = false;
    this:fire_trigger('on_close, ['Actor -> who]);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.close_msg):with_dobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    return true;
  endmethod

  method do_lock owner: ARCH_WIZARD
    "Core: lock this container with key.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_lock must be called by this object");
    const {who, key, ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    !this.locked && valid(key) && key.location == who || return false;
    const policy = this.lock_rule;
    policy != 0 && this:can_lock(who, key)['allowed] || return false;
    !this.locked && key.location == who && this.lock_rule == policy || return false;
    this.locked = true;
    this:fire_trigger('on_lock, ['Actor -> who, 'Key -> key]);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.lock_msg):with_dobj(key):with_iobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    return true;
  endmethod

  method do_unlock owner: ARCH_WIZARD
    "Core: unlock this container with key.";
    "Only callable by this object itself";
    caller != this && raise(E_PERM, "do_unlock must be called by this object");
    const {who, key, ?silent = false} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    this.locked && valid(key) && key.location == who || return false;
    const policy = this.unlock_rule;
    policy != 0 && this:can_unlock(who, key)['allowed] || return false;
    this.locked && key.location == who && this.unlock_rule == policy || return false;
    this.locked = false;
    this:fire_trigger('on_unlock, ['Actor -> who, 'Key -> key]);
    if (!silent && valid(who.location))
      const event = $event:mk_info(who, @this.unlock_msg):with_dobj(key):with_iobj(this):with_this(who.location);
      who.location:announce(event);
    endif
    return true;
  endmethod

  method action_take_from owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context, item} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_take_from(who, item);
  endmethod

  method action_put_into owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context, item} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_put_into(who, item);
  endmethod

  method action_open owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_open(who);
  endmethod

  method action_close owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_close(who);
  endmethod

  method action_lock owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context, key} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_lock(who, key);
  endmethod

  method action_unlock owner: ARCH_WIZARD
    "Apply the container action for its authenticated accessor.";
    const {who, context, key} = args;
    const actor = caller_perms();
    actor == who || (valid(actor) && actor.wizard) || raise(E_PERM);
    return this:do_unlock(who, key);
  endmethod

  method help_topics owner: ARCH_WIZARD
    "Return help topics for containers.";
    {for_player, ?topic = ""} = args;
    my_topics = {$help:mk("open", "Open a container", "Use 'open <container>' to open it and see its contents.", {}, 'commands, {"close", "put"}), $help:mk("close", "Close a container", "Use 'close <container>' to close it.", {"shut"}, 'commands, {"open"}), $help:mk("put", "Put something inside", "Use 'put <thing> in <container>' to place an object inside.", {"place"}, 'commands, {"get", "open"}), $help:mk("lock", "Lock a container", "Use 'lock <container> with <key>' to lock it.", {}, 'commands, {"unlock", "open"}), $help:mk("unlock", "Unlock a container", "Use 'unlock <container> with <key>' to unlock it.", {}, 'commands, {"lock", "open"})};
    topic == "" && return my_topics;
    for t in (my_topics)
      t:matches(topic) && return t;
    endfor
    return 0;
  endmethod
endobject
