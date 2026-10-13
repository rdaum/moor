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
  property view_rule (owner: HACKER, flags: "rc") = 0;
  property view_denied_msg (owner: HACKER, flags: "rc") = {"You cannot see inside that container."};
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
    "### Access rules",
    "",
    "A rule decides who can open, close, lock, unlock, take from, put into, or view a container.",
    "Use actor for the person who attempts the operation, this for the container, item for the item, and key for the key.",
    "",
    "Examples:",
    "",
    "```",
    "@rule chest take actor is me",
    "@rule chest open always",
    "@rule chest view actor is me",
    "@rule chest unlock key is \"brass key\"",
    "@rule chest open show",
    "@rule chest take clear",
    "```",
    "",
    "Clearing a rule restores default access. Containers have public open, close, view, take, and put access by default. Lock and unlock require a rule.",
    "",
    "Viewing and taking have separate rules. An item-specific take rule does not hide all the contents.",
    "",
    "Use @reaction help to make a container respond after an operation succeeds. Use @reaction events to see its event inputs.",
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

  method message_bindings owner: HACKER
    "Describe the participants and example values supplied to each editable message.";
    const {property} = args;
    if (tostr(property) in {"open_msg", "close_msg"})
      let bindings = $sub_utils:standard_bindings("The person opening or closing the container.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      bindings['iobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room where this happens.",
        'example -> 'location];
      return bindings;
    endif
    if (tostr(property) in {"put_msg", "take_msg"})
      let bindings = $sub_utils:standard_bindings("The person moving an item.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The item being moved.",
        'example -> 'item];
      bindings['iobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room where this happens.",
        'example -> 'location];
      bindings['prep] = ['kind -> 'text, 'description -> "The container preposition, such as in or on.",
        'example -> "in"];
      return bindings;
    endif
    if (tostr(property) in {"lock_msg", "unlock_msg"})
      let bindings = $sub_utils:standard_bindings("The person using the key.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The key.",
        'example -> 'item];
      bindings['iobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      bindings['this_obj] = ['kind -> 'object, 'description -> "The room where this happens.",
        'example -> 'location];
      return bindings;
    endif
    if (tostr(property) in {"already_open_msg", "already_closed_msg", "open_locked_msg", "already_locked_msg", "already_unlocked_msg"})
      let bindings = $sub_utils:standard_bindings("The person trying the operation.");
      bindings['dobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      return bindings;
    endif
    if (tostr(property) in {"put_denied_msg", "take_denied_msg", "open_denied_msg", "close_denied_msg", "lock_denied_msg", "unlock_denied_msg", "not_closeable_msg", "not_lockable_msg", "not_openable_msg", "not_unlockable_msg", "view_denied_msg"})
      let bindings = $sub_utils:standard_bindings("The person trying the operation.");
      bindings['iobj] = ['kind -> 'object, 'description -> "The container.",
        'example -> 'host];
      return bindings;
    endif
    return pass(@args);
  endmethod

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

  method can_lock owner: ARCH_WIZARD
    "Check the container's lock policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who, key} = args;
    const allowed = $container:_policy(this, 'lock, who, ['key -> key]);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.lock_denied_msg];
  endmethod

  method _log owner: ARCH_WIZARD
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this, {{"builtin_call", "server_log"}});
    server_log(@args);
  endmethod

  method can_unlock owner: ARCH_WIZARD
    "Check the container's unlock policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who, key} = args;
    const allowed = $container:_policy(this, 'unlock, who, ['key -> key]);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.unlock_denied_msg];
  endmethod

  method can_take_from owner: ARCH_WIZARD
    "Check the container's take policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who, item} = args;
    this.open || return ['allowed -> false, 'reason -> this.take_denied_msg];
    const allowed = $container:_policy(this, 'take, who, ['item -> item]);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.take_denied_msg];
  endmethod

  method can_put_into owner: ARCH_WIZARD
    "Check the container's put policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who, item} = args;
    this.open || return ['allowed -> false, 'reason -> this.put_denied_msg];
    const allowed = $container:_policy(this, 'put, who, ['item -> item]);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.put_denied_msg];
  endmethod

  method can_view_contents owner: ARCH_WIZARD
    "Apply the container's viewing rule and recheck reach/open state after authored callbacks.";
    set_task_perms(caller_perms());
    const {viewer} = args;
    const reachable = this.location == viewer || $thing:take_reachable(this, viewer);
    this.open && reachable || return false;
    const policy = this.view_rule;
    $container:_policy(this, 'view, viewer) || return false;
    equal(policy, this.view_rule) || return false;
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
      const denied = $event:mk_info(player, @this.view_denied_msg):with_iobj(this):resolve(player);
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
    return $container:_command(this, 'take, player, ['item -> dobj]);
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
    return $container:_command(this, 'put, player, ['item -> dobj]);
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
    return $container:_command(this, 'lock, player, ['key -> key]);
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
    return $container:_command(this, 'unlock, player, ['key -> key]);
  endverb

  method can_open owner: ARCH_WIZARD
    "Check the container's open policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who} = args;
    const allowed = $container:_policy(this, 'open, who, []);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.open_denied_msg];
  endmethod

  method can_close owner: ARCH_WIZARD
    "Check the container's close policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {who} = args;
    const allowed = $container:_policy(this, 'close, who, []);
    return ['allowed -> allowed, 'reason -> allowed ? {} | this.close_denied_msg];
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
    return $container:_command(this, 'open, player, []);
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
    return $container:_command(this, 'close, player, []);
  endverb

  method fact_is_open owner: HACKER
    "Rule predicate: Is this container open?";
    return this.open;
  endmethod

  method _command owner: ARCH_WIZARD
    "Run the shared operation and show a denied result to the person who issued the command.";
    set_task_perms(caller_perms());
    const {host, operation, who, arguments} = args;
    const result = $container:_operate(host, operation, who, arguments);
    if (result['status] == 'denied)
      const subject = `arguments['item] ! E_RANGE => `arguments['key] ! E_RANGE => host'';
      who:inform_current($event:mk_error(who, @result['reason]):with_dobj(subject):with_iobj(host));
    endif
    return result['status] == 'applied;
  endmethod

  method _reachable owner: HACKER
    "Check that the container is held by the actor or directly beside that actor.";
    const {host, who} = args;
    return host.location == who || (valid(who.location) && host.location == who.location);
  endmethod

  method _policy owner: ARCH_WIZARD
    "Evaluate the named container policy under the caller's read permissions.";
    set_task_perms(caller_perms());
    const {host, operation, who, ?arguments = []} = args;
    operation in {'take, 'put, 'open, 'close, 'lock, 'unlock, 'view} || raise(E_INVARG);
    let context = ['This -> host, 'Accessor -> who];
    if (operation in {'take, 'put})
      context['Dobj] = arguments['item];
    elseif (operation in {'lock, 'unlock})
      context['Key] = arguments['key];
    endif
    const policy = host.(tostr(operation) + "_rule");
    if (policy == 0)
      return !(operation in {'lock, 'unlock});
    endif
    return $rule_engine:evaluate(policy, context)['success];
  endmethod

  method _snapshot owner: ARCH_WIZARD
    "Capture custody, ownership, state, and policies that an operation must recheck after callbacks.";
    set_task_perms(caller_perms());
    const {host, who, arguments} = args;
    let snapshot = {host.owner, host.location, host.open, host.locked,
      host.take_rule, host.put_rule, host.open_rule, host.close_rule, host.lock_rule, host.unlock_rule,
      who.owner, who.location, who.wearing};
    if (maphaskey(arguments, 'item))
      const item = arguments['item];
      snapshot = {@snapshot, item.owner, item.location, item.portable, item.get_rule, item.drop_rule};
    endif
    if (maphaskey(arguments, 'key))
      const key = arguments['key];
      snapshot = {@snapshot, key.owner, key.location};
    endif
    return snapshot;
  endmethod

  method _check_operation owner: ARCH_WIZARD
    "Check rules and acceptance with the initiating principal's permissions, before mutation.";
    const {host, operation, who, arguments, principal} = args;
    const source = callers()[1];
    source[4] == $container && source[2] == "_operate" || raise(E_PERM);
    set_task_perms(principal);
    $container:_policy(host, operation, who, arguments)
      || return host.(tostr(operation) + "_denied_msg");
    if (operation in {'take, 'put})
      const item = arguments['item];
      if (operation == 'take)
        $thing:_policy(item, 'get, who) || return item.get_denied_msg;
        who:acceptable(item) || return {"You cannot carry that item."};
      else
        $thing:_policy(item, 'drop, who) || return item.drop_denied_msg;
        host:acceptable(item) || return {"That item does not fit here."};
      endif
    endif
    return {};
  endmethod

  method _operate owner: ARCH_WIZARD
    "Apply one container operation for an authenticated actor. Return status and denial reason.";
    "Policies and acceptance can suspend. Recheck the captured state before any mutation.";
    const {host, operation, who, ?arguments = [], ?silent = false, ?reaction_context = []} = args;
    const principal = caller_perms();
    $root:check_actor(who, principal);
    isa(host, $container) || raise(E_INVARG, "That object is not a container.");
    operation in {'take, 'put, 'open, 'close, 'lock, 'unlock} || raise(E_INVARG);
    const schema = operation in {'take, 'put} ? ['item -> 'object]
      | (operation in {'lock, 'unlock} ? ['key -> 'object] | []);
    $behavior_schema:bindings(arguments, schema, "action arguments");
    for argument in (mapvalues(arguments))
      valid(argument) || raise(E_INVARG, "An action argument no longer exists.");
    endfor
    const denied = {reason} => ['status -> 'denied, 'reason -> reason];
    $container:_reachable(host, who) || return denied({"That is no longer within reach."});
    if (operation in {'take, 'put})
      host.open || return denied(host.(tostr(operation) + "_denied_msg"));
      const source = operation == 'take ? host | who;
      arguments['item].location == source || return denied({"That item is no longer there."});
    elseif (operation == 'open)
      !host.open || return denied(host.already_open_msg);
      !host.locked || return denied(host.open_locked_msg);
    elseif (operation == 'close)
      host.open || return denied(host.already_closed_msg);
    else
      arguments['key].location == who || return denied({"You are not carrying that key."});
      if (operation == 'lock)
        !host.locked || return denied(host.already_locked_msg);
      else
        host.locked || return denied(host.already_unlocked_msg);
      endif
    endif
    const before = $container:_snapshot(host, who, arguments);
    const reason = $container:_check_operation(host, operation, who, arguments, principal);
    length(reason) == 0 || return denied(reason);
    $root:check_actor(who, principal);
    equal(before, $container:_snapshot(host, who, arguments))
      || return denied({"The objects or their rules changed. Try again."});
    const location = who.location;
    let context = ['Actor -> who, 'Location -> location,
      '_reaction_chain -> `reaction_context['_reaction_chain] ! E_RANGE => {}'];
    if (operation in {'take, 'put})
      const item = arguments['item];
      const destination = operation == 'take ? who | host;
      move(item, destination);
      item.location == destination || raise(E_INVARG, "The item did not reach its destination.");
      context['Item] = item;
      $reaction:_dispatch(item, operation == 'take ? 'on_get | 'on_drop, context);
    else
      const property = operation in {'open, 'close} ? "open" | "locked";
      const old_value = host.(property);
      const new_value = !!(operation in {'open, 'lock});
      host.(property) = new_value;
      $reaction:_dispatch(host, 0, context, {tosym(property), old_value, new_value});
      if (maphaskey(arguments, 'key))
        context['Key] = arguments['key];
      endif
    endif
    $reaction:_dispatch(host, tosym("on_" + tostr(operation)), context);
    set_task_perms(principal);
    if (!silent && valid(location))
      let event = $event:mk_info(who, @host.(tostr(operation) + "_msg")):with_this(location);
      if (operation in {'take, 'put})
        event = event:with_dobj(arguments['item]):with_iobj(host)
          :with_text_binding('prep, host.put_prep_display);
      elseif (operation in {'lock, 'unlock})
        event = event:with_dobj(arguments['key]):with_iobj(host);
      else
        event = event:with_dobj(host):with_iobj(host);
      endif
      location:announce(event);
    endif
    return ['status -> 'applied, 'reason -> {}];
  endmethod

  method do_take_from owner: ARCH_WIZARD
    "Apply take through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, item, ?silent = false} = args;
    return $container:_operate(this, 'take, who, ['item -> item], silent)['status] == 'applied;
  endmethod

  method do_put_into owner: ARCH_WIZARD
    "Apply put through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, item, ?prep = "", ?silent = false} = args;
    return $container:_operate(this, 'put, who, ['item -> item], silent)['status] == 'applied;
  endmethod

  method do_open owner: ARCH_WIZARD
    "Apply open through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, ?silent = false} = args;
    return $container:_operate(this, 'open, who, [], silent)['status] == 'applied;
  endmethod

  method do_close owner: ARCH_WIZARD
    "Apply close through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, ?silent = false} = args;
    return $container:_operate(this, 'close, who, [], silent)['status] == 'applied;
  endmethod

  method do_lock owner: ARCH_WIZARD
    "Apply lock through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, key, ?silent = false} = args;
    return $container:_operate(this, 'lock, who, ['key -> key], silent)['status] == 'applied;
  endmethod

  method do_unlock owner: ARCH_WIZARD
    "Apply unlock through the common container operation; return whether it succeeded.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    const {who, key, ?silent = false} = args;
    return $container:_operate(this, 'unlock, who, ['key -> key], silent)['status] == 'applied;
  endmethod

  method action_take_from owner: ARCH_WIZARD
    "Apply take for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context, item} = args;
    return $container:_operate(this, 'take, who, ['item -> item], false, context)['status] == 'applied;
  endmethod

  method action_put_into owner: ARCH_WIZARD
    "Apply put for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context, item} = args;
    return $container:_operate(this, 'put, who, ['item -> item], false, context)['status] == 'applied;
  endmethod

  method action_open owner: ARCH_WIZARD
    "Apply open for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    return $container:_operate(this, 'open, who, [], false, context)['status] == 'applied;
  endmethod

  method action_close owner: ARCH_WIZARD
    "Apply close for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context} = args;
    return $container:_operate(this, 'close, who, [], false, context)['status] == 'applied;
  endmethod

  method action_lock owner: ARCH_WIZARD
    "Apply lock for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context, key} = args;
    return $container:_operate(this, 'lock, who, ['key -> key], false, context)['status] == 'applied;
  endmethod

  method action_unlock owner: ARCH_WIZARD
    "Apply unlock for the caller or an actor they own, with normal policies and triggers.";
    set_task_perms(caller_perms());
    const {who, context, key} = args;
    return $container:_operate(this, 'unlock, who, ['key -> key], false, context)['status] == 'applied;
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
