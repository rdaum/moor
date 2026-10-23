object ACTOR [
  import_export_id -> "actor"
]
  name: "Generic Actor"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  fertile: true
  readable: true

  property listing_form (owner: HACKER, flags: "rc") = 'indefinite;
  property performing (owner: ARCH_WIZARD, flags: "rc") = "{}";
  property pronouns (owner: ARCH_WIZARD, flags: "rc") = <PRONOUNS, .verb_be = "are", .verb_have = "have", .is_plural = true, .display = "they/them", .ps = "they", .po = "them", .pp = "their", .pq = "theirs", .pr = "themselves">;
  property wearing (owner: ARCH_WIZARD, flags: "rc") = {};

  override description (owner: ARCH_WIZARD, flags: "rc") = "Generic actor prototype providing core behavior for NPCs and players including item transfer, communication, and movement.";

  method reference_kind owner: HACKER
    "Identify players and other actors as people in client references.";
    return "person";
  endmethod

  method is_actor owner: HACKER
    "Actors can perform actions in the world.";
    return true;
  endmethod

  method acceptable owner: HACKER
    "Default: actors accept items.";
    return true;
  endmethod

  verb put (any in this) owner: HACKER flags: "rd"
    "Reject putting things in an actor";
    event = $event:mk_error(player, $sub:thec('this), " ", $sub:agree('this, 'name, "are", "is"), " a person, not a container."):with_this(this);
    player:inform_current(event);
  endverb

  verb "give hand" (any at this) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["arguments" -> ["dobj" -> "Item"], "prefix" -> "Give to"]
  ]
    "Give a carried object to a nearby actor through the command parser.";
    const actor = caller_perms();
    actor == #-1 && caller == player || actor == player || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(actor == #-1 ? player | actor);
    if (!dobjstr)
      player:inform_current($event:mk_error(player, "Give what?"));
      return;
    endif
    try
      dobj = $match:match_object(dobjstr, player);
    except (ANY)
      player:inform_current($event:mk_error(player, "You don't have that."));
      return;
    endtry
    try
      this:do_give(player, dobj);
    except e (E_PERM)
      player:inform_current($event:mk_error(player, e[2]));
    endtry
  endverb

  method do_give owner: ARCH_WIZARD
    "Transfer a held item without requiring its giver to own it.";
    caller == this || raise(E_PERM, "do_give must be called by this object");
    const {who, item} = args;
    const actor = caller_perms();
    valid(who) && (actor == who || (valid(actor) && actor.wizard)) || raise(E_PERM);
    valid(item) && item.location == who || raise(E_PERM, "You don't have that to give.");
    this != who || raise(E_PERM, "You already have that.");
    const room = who.location;
    valid(room) && this.location == room || raise(E_PERM, "That recipient is no longer within reach.");
    const is_thing = isa(item, $thing);
    const policy = is_thing ? item.drop_rule | 0;
    if (is_thing && !item:can_drop(who))
      who:inform_current($event:mk_error(who, @item.drop_denied_msg):with_dobj(item));
      return false;
    endif
    this:acceptable(item) || raise(E_PERM, "That recipient can't accept the item.");
    "Policy callbacks may suspend; possession and proximity must still hold.";
    valid(item) && item.location == who || raise(E_PERM, "You no longer have that to give.");
    valid(who) && who.location == room && this.location == room || raise(E_PERM, "That recipient is no longer within reach.");
    !is_thing || item.drop_rule == policy || raise(E_PERM, "The item's access rules changed. Please try again.");
    const moved = item:moveto(this);
    typeof(moved) == TYPE_ERR && raise(moved);
    item.location == this || raise(E_INVARG, "Item movement did not reach its destination");
    const event = $event:mk_info(who, $sub:thec('actor), " ", $sub:agree('actor, 'name, "give", "gives"), " ", $sub:the('dobj), " to ", $sub:the('iobj), "."):with_dobj(item):with_iobj(this):with_this(room);
    room:announce(event);
    return true;
  endmethod

  verb "get take steal grab" (any from this) owner: ARCH_WIZARD flags: "rd"
    "Take an object from this actor";
    caller == player || raise(E_PERM);
    set_task_perms(player);
    if (!dobjstr || dobjstr == "")
      event = $event:mk_error(player, "Take what?");
      player:inform_current(event);
      return;
    endif
    "Match the object from dobjstr - search in this container's contents";
    try
      dobj = $match:resolve_in_scope(dobjstr, this.contents);
    except e (ANY)
      event = $event:mk_error(player, $sub:thec('this), " ", $sub:agree('this, 'name, "don't", "doesn't"), " have that."):with_this(this);
      player:inform_current(event);
      return;
    endtry
    if (!valid(dobj) || typeof(dobj) != TYPE_OBJ)
      event = $event:mk_error(player, $sub:thec('this), " ", $sub:agree('this, 'name, "don't", "doesn't"), " have that."):with_this(this);
      player:inform_current(event);
      return;
    endif
    if (dobj.location != this)
      event = $event:mk_error(player, $sub:thec('dobj), " ", $sub:agree('dobj, 'name, "aren't", "isn't"), " with ", $sub:the('this), "."):with_dobj(dobj):with_this(this);
      player:inform_current(event);
      return;
    endif
    if (this == player)
      event = $event:mk_error(player, "You already have ", $sub:the('dobj), "."):with_dobj(dobj);
      player:inform_current(event);
      return;
    endif
    "Check if player can accept the item";
    if (!player:acceptable(dobj))
      event = $event:mk_error(player, "You can't carry ", $sub:the('dobj), "."):with_dobj(dobj);
      player:inform_current(event);
      return;
    endif
    "Try to move it";
    try
      dobj:moveto(player);
    except e (E_PERM)
      msg = length(e) > 2 ? e[2] | "You can't take that from " + this:noun_phrase('definite, player) + ".";
      event = $event:mk_error(player, msg):with_dobj(dobj);
      player:inform_current(event);
      return;
    endtry
    "Announce to room";
    if (valid(player.location))
      event = $event:mk_info(player, $sub:thec('actor), " ", $sub:agree('actor, 'name, "take", "takes"), " ", $sub:the('dobj), " from ", $sub:the('iobj), "."):with_dobj(dobj):with_iobj(this):with_this(player.location);
      player.location:announce(event);
    endif
  endverb

  method mk_emote_event owner: HACKER
    "Use a third-person subject; the authored continuation is not conjugated.";
    return $event:mk_emote(this, $sub:attribution(this), " ", @$format.djot:inline(args[1])):with_this(this.location);
  endmethod

  method mk_say_event owner: HACKER
    "Parse authored speech before combining it with literal attribution.";
    let event = $event:mk_say(this, $sub:thec('actor), " ", $sub:agree('actor, 'name, "say", "says"), ", \"", @$format.djot:inline(args[1]), "\""):with_this(this.location);
    event = event:with_metadata('content, args[1]);
    event = event:with_presentation_hint('speech_bubble);
    return event;
  endmethod

  method mk_directed_say_event owner: HACKER
    "Directed say: 'Name [to Target]: message'";
    {target, message} = args;
    return $event:mk_directed_say(this, this:name(), " [to ", $sub:the('iobj), "]: ", @$format.djot:inline(message)):with_iobj(target):with_this(this.location);
  endmethod

  method mk_think_event owner: HACKER
    "Thoughts always show the actor's name, never 'You'";
    event = $event:mk_think(this, this:name(), " . o O ( ", @$format.djot:inline(args[1]), " )"):with_this(this.location);
    event = event:with_metadata('content, args[1]);
    event = event:with_presentation_hint('thought_bubble);
    return event;
  endmethod

  method mk_connected_event owner: HACKER
    "Create a connection announcement event.";
    "Args: ?is_new_player = false";
    {?is_new_player = false} = args;
    "Select template based on whether this is a new player";
    if (is_new_player)
      template = `$login.new_player_arrival_template ! E_PROPNF => "{The actor} {agree actor name:have|has} just arrived."';
    else
      template = `$login.player_wakeup_template ! E_PROPNF => "{The actor} {agree actor name:have|has} woken up."';
    endif
    "Compile the template into $sub flyweights";
    content = $sub_utils:compile(template);
    return $event:mk_say(this, @content):with_presentation_hint('inset):with_group('connection, this);
  endmethod

  method mk_disconnected_event owner: HACKER
    return $event:mk_say(this, $sub:thec('actor), " ", $sub:agree('actor, 'name, "go", "goes"), " to sleep."):with_presentation_hint('inset):with_group('connection, this);
  endmethod

  method mk_departure_event owner: HACKER
    "Create a departure event. Optional departure_phrase overrides default template.";
    {from_room, ?direction = "", ?passage_desc = "", ?to_room = #-1, ?departure_phrase = ""} = args;
    typeof(direction) == TYPE_STR || (direction = "");
    typeof(passage_desc) == TYPE_STR || (passage_desc = "");
    typeof(departure_phrase) == TYPE_STR || (departure_phrase = "");
    parts = {$sub:thec('actor), " ", $sub:agree('actor, 'name, "head", "heads")};
    if (departure_phrase)
      "Use custom departure phrase if provided";
      parts = {@parts, " ", departure_phrase};
    else
      "Default: construct from direction and passage description";
      if (direction)
        parts = {@parts, " ", direction};
      else
        parts = {@parts, " out"};
      endif
      if (passage_desc)
        passage_desc = $sub:phrase(passage_desc, {'strip_period, 'initial_lowercase});
        parts = {@parts, " through ", passage_desc};
      endif
    endif
    parts = {@parts, "."};
    event = $event:mk_move(this, @parts);
    valid(from_room) && (event = event:with_this(from_room):with_location(from_room));
    valid(to_room) && (event = event:with_iobj(to_room));
    return event;
  endmethod

  method mk_arrival_event owner: HACKER
    "Create an arrival event. Optional arrival_phrase overrides default template.";
    {to_room, ?direction = "", ?passage_desc = "", ?from_room = #-1, ?arrival_phrase = ""} = args;
    typeof(direction) == TYPE_STR || (direction = "");
    typeof(passage_desc) == TYPE_STR || (passage_desc = "");
    typeof(arrival_phrase) == TYPE_STR || (arrival_phrase = "");
    parts = {$sub:thec('actor), " ", $sub:agree('actor, 'name, "arrive", "arrives")};
    if (arrival_phrase)
      "Use custom arrival phrase if provided";
      parts = {@parts, " ", arrival_phrase};
    else
      "Default: construct from direction and passage description";
      if (direction)
        "The direction is this room's exit back the way the actor came, so up means from above.";
        const places = ["up" -> "above", "down" -> "below", "upstairs" -> "upstairs",
          "downstairs" -> "downstairs", "in" -> "inside", "inside" -> "inside",
          "out" -> "outside", "outside" -> "outside"];
        const place = maphaskey(places, direction:lowercase()) ? places[direction:lowercase()] | 0;
        parts = {@parts, " from ", place ? place | "the " + direction};
      endif
      if (passage_desc)
        passage_desc = $sub:phrase(passage_desc, {'strip_period, 'initial_lowercase});
        parts = {@parts, ", emerging from ", passage_desc};
      endif
    endif
    parts = {@parts, "."};
    event = $event:mk_move(this, @parts);
    valid(to_room) && (event = event:with_this(to_room):with_location(to_room));
    valid(from_room) && (event = event:with_iobj(from_room));
    return event;
  endmethod

  method test_arrival_direction_names_the_origin owner: ARCH_WIZARD
    "The direction is the exit back toward the origin: arriving by up stairs means coming from below.";
    for case in ({{"down", "below"}, {"up", "above"}, {"out", "outside"}, {"in", "inside"},
        {"upstairs", "upstairs"}, {"lobby", "the lobby"}, {"north", "the north"}})
      const {direction, place} = case;
      const text = toliteral($actor:mk_arrival_event(#-1, direction));
      $test_utils:assert_true(index(text, ".text = \" from \"") && index(text, ".text = \"" + place + "\""),
        "arrival via " + direction + " reads from " + place);
    endfor
    return true;
  endmethod

  method pronouns owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return $pronouns:validate(this.pronouns);
  endmethod

  method "pronoun_*" owner: ARCH_WIZARD
    "Read a pronoun form; possessive takes one selector, 'adj or 'noun.";
    set_task_perms(caller_perms());
    return $pronouns:form(this:pronouns(), tosym(verb[9..$]), @args);
  endmethod

  method fact_is_wizard owner: HACKER
    "Fact predicate: Is this actor a wizard?";
    {actor} = args;
    return actor.wizard;
  endmethod

  method fact_is_programmer owner: HACKER
    "Fact predicate: Does this actor have programmer privileges?";
    {actor} = args;
    return actor.programmer;
  endmethod

  method fact_is_builder owner: ARCH_WIZARD
    "Fact predicate: Does this actor have builder privileges?";
    {?actor = this} = args;
    return `actor.is_builder ! E_PROPNF => false';
  endmethod

  method fact_has_in_inventory owner: HACKER
    "Fact predicate: Does this actor have thing in their inventory?";
    {actor, thing} = args;
    return thing.location == actor;
  endmethod

  method fact_owns owner: HACKER
    "Fact predicate: Does this actor own thing?";
    {actor, thing} = args;
    return thing.owner == actor;
  endmethod

  method action_go owner: ARCH_WIZARD
    "Move this actor through a local passage under the invoking principal's authority.";
    const principal = caller_perms();
    const {who, context, direction} = args;
    $root:check_actor(this, principal);
    who == this || raise(E_INVARG, "The action actor does not match its receiver.");
    set_task_perms(principal);
    valid(this.location) || return false;
    return this.location:action_go(this, context, direction);
  endmethod

  method mk_stagetalk owner: ARCH_WIZARD
    "Stagetalk: directed speech 'Name [to Target]: message'";
    {target, message} = args;
    return $event:mk_stagetalk(this, this:name(), " [to ", $sub:the('iobj), "]: ", @$format.djot:inline(message)):with_iobj(target):with_this(this.location);
  endmethod

  method pronouns_display owner: ARCH_WIZARD
    "Return this actor's pronoun label, such as he/him or they/them.";
    return $pronouns:display(this.pronouns);
  endmethod

  method set_pronouns owner: ARCH_WIZARD
    "Set a preset or validated custom fields. Commands and clients use this same permission check.";
    let actor = caller_perms();
    "Direct client calls have no MOO caller; nested and scheduled calls retain their own authority.";
    if (actor == #-1 && !callers() && caller == player)
      actor = player;
    endif
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_pronouns);
    set_task_perms(perms);
    const {input} = args;
    target.pronouns = $pronouns:from_input(input);
  endmethod

  method pronouns_settings owner: ARCH_WIZARD
    "Return the current words for the client's pronoun editor.";
    return $pronouns:to_input(this.pronouns);
  endmethod

  method mk_shout_event owner: ARCH_WIZARD
    "Create a shout event with loudness for acoustic propagation.";
    {text} = args;
    event = $event:mk_shout(this, $sub:thec('actor), " ", $sub:agree('actor, 'name, "shout", "shouts"), ", \"", @$format.djot:inline(text), "\""):with_this(this.location);
    event = event:with_metadata('content, text);
    event = event:with_metadata('loudness, 5);
    event = event:with_presentation_hint('speech_bubble);
    return event;
  endmethod

  method action_start_activity owner: ARCH_WIZARD
    "Action handler: record an active task for an actor.";
    set_task_perms(this.owner);
    {who, kind, task_id, ?label = ""} = args;
    who != this && return this.performing;
    if (!(typeof(task_id) == TYPE_INT && task_id > 0))
      return this.performing;
    endif
    if (!(typeof(this.performing) == TYPE_LIST))
      this.performing = {};
    endif
    this:action_clear_activity_task(this, task_id);
    entry = $player_activity:make_entry(kind, task_id, label);
    this.performing = {@this.performing, entry};
    return this.performing;
  endmethod

  method action_clear_activity_task owner: ARCH_WIZARD
    "Action handler: remove activity entries for a task id.";
    set_task_perms(this.owner);
    {who, task_id} = args;
    who != this && return {};
    if (!(typeof(this.performing) == TYPE_LIST))
      this.performing = {};
      return this.performing;
    endif
    kept = {};
    for entry in (this.performing)
      entry_task = $player_activity:task_id_of(entry);
      if (entry_task != task_id)
        kept = {@kept, entry};
      endif
    endfor
    this.performing = kept;
    return kept;
  endmethod

  method action_stop_activities owner: ARCH_WIZARD
    "Action handler: cancel activities, optionally by kind.";
    set_task_perms(this.owner);
    {who, ?kind = $nothing} = args;
    who != this && return {};
    if (!(typeof(this.performing) == TYPE_LIST))
      this.performing = {};
      return {};
    endif
    kept = {};
    canceled = {};
    for entry in (this.performing)
      entry_kind = $player_activity:kind_of(entry);
      if (kind != $nothing && entry_kind != kind)
        kept = {@kept, entry};
        continue;
      endif
      if ($player_activity:cancel_entry(entry))
        canceled = {@canceled, entry};
      endif
    endfor
    this.performing = kept;
    return canceled;
  endmethod

  method action_activity_descriptions owner: ARCH_WIZARD
    "Action handler: describe current activities for display.";
    set_task_perms(this.owner);
    {who} = args;
    who != this && return {};
    if (!(typeof(this.performing) == TYPE_LIST) || length(this.performing) == 0)
      return {};
    endif
    descriptions = {};
    for entry in (this.performing)
      description = $player_activity:description_of(entry);
      if (typeof(description) == TYPE_STR && description != "" && !(description in descriptions))
        descriptions = {@descriptions, description};
      endif
    endfor
    return descriptions;
  endmethod

  method look_self owner: ARCH_WIZARD
    "Include shared clothing and activity details when looking at an NPC.";
    set_task_perms(caller_perms());
    const view = pass(@args);
    let slots = flyslots(view);
    slots['description] = {view.description, @this:_look_self_details()};
    return toflyweight(view.delegate, slots, flycontents(view));
  endmethod

  method _look_self_details owner: ARCH_WIZARD
    "Describe clothing with object references and retain authored activity descriptions.";
    set_task_perms(caller_perms());
    let details = {};
    if (typeof(this.wearing) == TYPE_LIST && length(this.wearing))
      const items = {item for item in (this.wearing) if valid(item)};
      let clothing = {};
      let integrated = {};
      for i in [1..length(items)]
        i > 1 && (clothing = {@clothing, i == length(items) ? " and " | ", "});
        const item = items[i];
        clothing = {@clothing, $sub:reference(item, 'listing)};
        if (respond_to(item, 'integrate_description))
          const description = item:integrate_description();
          if (typeof(description) == TYPE_STR && length(description))
            integrated = {@integrated, description};
          endif
        endif
      endfor
      if (length(clothing))
        details = {@details, " ", $sub:sc_dobj(), " ", $sub:verb_be_dobj(),
          " wearing ", @clothing, "."};
      endif
      for description in (integrated)
        details = {@details, " ", description};
      endfor
    endif
    const activities = this:action_activity_descriptions(this);
    if (length(activities))
      details = {@details, "\n\n", $sub:sc_dobj(), " ", $sub:verb_be_dobj(),
        " currently ", activities:english_list(), "."};
    endif
    return details;
  endmethod
endobject
