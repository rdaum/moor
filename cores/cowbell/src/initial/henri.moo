object HENRI [
  import_export_id -> "henri",
  import_export_hierarchy -> {"initial"}
]
  name: "Henri"
  parent: ACTOR
  location: FIRST_ROOM
  owner: HACKER
  readable: true

  property behaviours_disabled (owner: HACKER, flags: "rc") = true;
  property complaints_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'actor>,
      " notes, \"The dust gets everywhere. Absolutely everywhere.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " complains, \"They moved my favorite sunbeam spot. The audacity.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " grumbles, \"The hammering is giving me a headache. And I have very sensitive ears, you know.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " sighs, \"This used to be a perfectly good nap spot before they started all this construction.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " fusses, \"Do you know how hard it is to maintain this level of fur quality with all this sawdust floating around?\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " reminisces, \"My previous accommodations had much better acoustics for my midnight operas.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " mutters, \"They keep moving the furniture. It's very disorienting for a creature of routine.\""
    }}>;
  property couch_intruder_reaction (owner: HACKER, flags: "r") = 0;
  property cupboard_open_reaction (owner: HACKER, flags: "r") = 0;
  property feed_rule (owner: HACKER, flags: "rc") = 0;
  property kibble_taken_reaction (owner: HACKER, flags: "r") = 0;
  property look_self_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'actor>,
      " glares at you with an expression that clearly says 'This is all ",
      <SUB, .capitalize = false, .type = 'self_alt, .for_self = "your", .for_others = "their">,
      " fault, isn't it?' ",
      <SUB, .capitalize = false, .type = 'self_alt, .for_self = "p", .for_others = "Their">,
      " tail twitches irritably."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " is trying to nap despite the construction noise, but keeps getting interrupted. You can see the sleep-deprived annoyance in their half-closed eyes."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " seems mildly interested in something despite the general annoyance. Probably wondering if you brought treats or if you're just another source of disappointment."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " gives you a look that manages to convey both disdain and resignation about their current living conditions."
    }}>;
  property mood (owner: HACKER, flags: "rc") = "grouchy";
  property pet_rule (owner: HACKER, flags: "rc") = 0;
  property pet_attempts (owner: HACKER, flags: "rc") = 0;
  property scheduled_behaviours (owner: HACKER, flags: "rc") = [];
  property sleepy_threshold_reaction (owner: HACKER, flags: "r") = 0;

  override aliases (owner: HACKER, flags: "rc") = {"cat", "grouchy cat"};
  override description (owner: HACKER, flags: "rc") = "A sleek black cat with piercing green eyes and an air of perpetual annoyance. His fur is immaculately groomed despite the construction dust, and he holds himself with the offended dignity of a creature who knows he deserves better accommodations. He occasionally flicks his tail in irritation, as if to emphasize his displeasure with the current state of affairs.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Henri",
    "",
    "Henri demonstrates rules and reactions that builders can inspect and edit without writing code.",
    "He starts grouchy with zero pet attempts. Each nearby attempt counts, including attempts he refuses. On the fourth attempt he becomes sleepy, and the counter resets.",
    "His pet_attempt reactions describe refused grouchy attempts. His pet reactions describe successful petting. Feeding accepted kibble makes him sleepy and resets the counter.",
    "All interaction responses are installed together by initialize through the same @reaction, @state, and @rule services used by builders. The pet and feed methods only check and report the actual operations.",
    "Use @reaction Henri to list responses, @reaction Henri NAME show to read every step, and @state Henri to inspect mood and the counter. Editing requires ownership or wizard privileges.",
    "Copied reactions use local counters and named settings. The couch setting identifies the seat; it is not a hidden global in an action.",
    "The optional native idle schedules remain disabled by default. They supply background grooming and other idle actions separately from the interaction reactions."
  };

  override pronouns (owner: HACKER, flags: "rc") = <PRONOUNS, .verb_be = "is", .verb_have = "has", .is_plural = false, .display = "he/him", .ps = "he", .po = "him", .pp = "his", .pq = "his", .pr = "himself">;

  method _pick_message owner: HACKER
    "Pick a message from a bag/string or use a compiled list directly, returning empty string on failure.";
    {source} = args;
    if ($msg_bag:is_msg_bag(source))
      msg = source:pick();
      return typeof(msg) == TYPE_ERR ? "" | msg;
    elseif (typeof(source) == TYPE_LIST)
      "Assume this is a compiled template list; return as-is";
      return source;
    elseif (typeof(source) == TYPE_STR)
      return source;
    endif
    return "";
  endmethod

  method can_be_petted owner: ARCH_WIZARD
    "Check the current pet policy under the caller's permissions. Reactions choose mood responses.";
    set_task_perms(caller_perms());
    const {accessor} = args;
    const allowed = this.pet_rule == 0 || $rule_engine:evaluate(this.pet_rule,
      ['This -> this, 'Accessor -> accessor])['success];
    return ['allowed -> allowed, 'reason -> allowed ? {} | {"Henri does not let you pet him."}];
  endmethod

  method can_be_fed owner: ARCH_WIZARD
    "Check the current food policy under the caller's permissions.";
    set_task_perms(caller_perms());
    const {accessor, food} = args;
    const allowed = this.feed_rule == 0 || $rule_engine:evaluate(this.feed_rule,
      ['This -> this, 'Accessor -> accessor, 'Food -> food])['success];
    return ['allowed -> allowed, 'reason -> allowed ? {} | {"Henri will not eat that."}];
  endmethod

  verb "pet stroke" (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Pet Henri. An attempt event precedes the policy check; a successful pet emits its own event.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || (valid(principal) && principal.wizard))
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    const result = $henri:_pet(this, player);
    if (result['status] == 'denied)
      player:inform_current($event:mk_error(player, @result['reason]));
    endif
    return result['status] == 'applied;
  endverb

  method _pet owner: ARCH_WIZARD
    "Authenticate a nearby pet attempt and emit exactly one event for its checked outcome.";
    set_task_perms(caller_perms());
    const {cat, who} = args;
    $root:check_actor(who);
    isa(cat, $henri) || raise(E_INVARG, "This petting operation requires a cat descended from Henri.");
    const denied = {reason} => ['status -> 'denied, 'reason -> {reason}];
    (valid(who.location) && cat.location == who.location) || return denied("Henri is not within reach.");
    const room = who.location;
    const context = ['Actor -> who, 'Location -> room];
    $reaction:_dispatch(cat, 'on_pet_attempt, context);
    const policy = cat.pet_rule;
    const owner = cat.owner;
    const allowed = policy == 0 || $rule_engine:evaluate(policy, ['This -> cat, 'Accessor -> who])['success];
    $root:check_actor(who);
    (cat.location == room && who.location == room && cat.owner == owner && equal(cat.pet_rule, policy))
      || return denied("The cat or the petting rule changed. Try again.");
    allowed || return denied("Henri does not let you pet him.");
    room:announce($event:mk_emote(who, $sub:nc(), " ", $sub:self_alt("pet", "pets"),
      " ", $sub:the('dobj), "."):with_dobj(cat));
    $reaction:_dispatch(cat, 'on_pet, context);
    return ['status -> 'applied, 'reason -> {}];
  endmethod

  verb "feed give" (this with any) owner: ARCH_WIZARD flags: "rxd"
    "Offer carried food to Henri. The completed event supplies actor and food to his reactions.";
    const principal = caller_perms();
    ((principal == #-1 && caller == player) || principal == player || (valid(principal) && principal.wizard))
      || raise(E_PERM);
    set_task_perms(principal == #-1 ? player | principal);
    try
      length(iobjstr) > 0 || raise(E_INVARG, "Feed Henri with what?");
      const food = $match:match_object(iobjstr, player);
      const result = $henri:_feed(this, player, food);
      if (result['status] == 'denied)
        player:inform_current($event:mk_error(player, @result['reason]));
      endif
      return result['status] == 'applied;
    except problem (E_INVARG, E_PERM)
      player:inform_current($event:mk_error(player, problem[2]));
      return false;
    endtry
  endverb

  method _feed owner: ARCH_WIZARD
    "Accept carried food after actor, custody, item policy, and cat policy checks. Reactions own the response.";
    const principal = caller_perms();
    const {cat, who, food} = args;
    $root:check_actor(who, principal);
    isa(cat, $henri) && isa(food, $thing) || raise(E_INVARG, "Supply a cat and a carried food item.");
    const denied = {reason} => ['status -> 'denied, 'reason -> {reason}];
    (valid(who.location) && cat.location == who.location && food.location == who)
      || return denied("Bring the food within reach of Henri first.");
    const before = {cat.owner, cat.location, cat.feed_rule, who.owner, who.location,
      food.owner, food.location, food.drop_rule};
    $henri:_feeding_allowed(cat, who, food, principal) || return denied("Henri will not eat that.");
    $root:check_actor(who, principal);
    equal(before, {cat.owner, cat.location, cat.feed_rule, who.owner, who.location,
      food.owner, food.location, food.drop_rule}) || return denied("The food, cat, or their rules changed. Try again.");
    const room = who.location;
    move(food, cat);
    food.location == cat || raise(E_INVARG, "The food did not reach the cat.");
    set_task_perms(principal);
    room:announce($event:mk_emote(who, $sub:nc(), " ", $sub:self_alt("offer", "offers"),
      " ", $sub:i(), " to ", $sub:the('dobj), "."):with_dobj(cat):with_iobj(food));
    $reaction:_dispatch(cat, 'on_fed, ['Actor -> who, 'Food -> food, 'Location -> room]);
    return ['status -> 'applied, 'reason -> {}];
  endmethod

  method _feeding_allowed owner: ARCH_WIZARD
    "Evaluate authored food policies and destination acceptance under the initiating principal.";
    const source = callers()[1];
    source[4] == $henri && source[2] == "_feed" || raise(E_PERM);
    const {cat, who, food, principal} = args;
    set_task_perms(principal);
    $thing:_policy(food, 'drop, who) || return false;
    cat:acceptable(food) || return false;
    return cat.feed_rule == 0 || $rule_engine:evaluate(cat.feed_rule,
      ['This -> cat, 'Accessor -> who, 'Food -> food])['success];
  endmethod

  method look_self owner: HACKER
    look_data = pass(@args);
    base_desc = look_data.description;
    desc_parts = typeof(base_desc) == TYPE_LIST ? base_desc | {base_desc};
    mood_extra = this:_pick_message(this.look_self_msg_bag);
    if (typeof(mood_extra) != TYPE_LIST)
      mood_extra = {mood_extra};
    endif
    merged = mood_extra ? {@desc_parts, "\n", @mood_extra} | desc_parts;
    return <look_data.delegate, .what = look_data.what, .title = look_data.title, .description = merged>;
  endmethod

  method start_behaviours owner: HACKER
    "Start Henri's six adaptive native schedules. The owner or a wizard may change this policy.";
    caller_perms().wizard || caller_perms() == this.owner || raise(E_PERM);
    this.behaviours_disabled = false;
    const started = this:_install_behaviour_schedules();
    player:inform_current($event:mk_info(player, started ? "Henri's autonomous behaviours started." | "Henri's behaviours are already running."));
    if (started && valid(this.location))
      this.location:announce(this:mk_emote_event("seems to settle into a routine of periodic sulking and grooming."));
    endif
    return started;
  endmethod

  method stop_behaviours owner: HACKER
    "Cancel future behaviours as the owner or a wizard. Running callbacks finish; cancellation commits with this state.";
    caller_perms().wizard || caller_perms() == this.owner || raise(E_PERM);
    this.behaviours_disabled = true;
    for schedule_id, behaviour_name in (this.scheduled_behaviours)
      schedule_stop(schedule_id);
    endfor
    this.scheduled_behaviours = [];
    player:inform_current($event:mk_info(player, "Henri's autonomous behaviours stopped."));
    if (valid(this.location))
      this.location:announce(this:mk_emote_event("seems to settle into a more permanent state of annoyance."));
    endif
    return true;
  endmethod

  method _autonomous_groom owner: HACKER
    "Autonomous grooming behaviour - called periodically by scheduler";
    "Only announce if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Generate grooming behaviour with grouchy commentary";
    grooming_behaviours = {"meticulously cleans one paw, then gives it a look of profound disappointment. 'The dust in this place is simply unacceptable.'", "attends to his fur with fastidious care, muttering about 'substandard grooming conditions.'", "washes his face with an expression that says 'I shouldn't have to work this hard to maintain basic standards.'", "grooms his sleek black coat, occasionally pausing to flick construction dust away with visible irritation.", "cleans behind his ears with the weary air of someone performing an unpleasant but necessary chore.", "smooths his fur, then sighs dramatically. 'My previous establishment had much better air filtration.'"};
    behaviour = grooming_behaviours[random(length(grooming_behaviours))];
    "Announce to room";
    this.location:announce(this:mk_emote_event(behaviour));
  endmethod

  method _autonomous_stretch owner: HACKER
    "Autonomous stretching behaviour - called periodically by scheduler";
    "Only announce if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Generate stretching behaviour with complaints";
    stretching_behaviours = {"arches his back in a long, luxurious stretch, then gives the floor a disapproving look. 'Too hard for proper stretching.'", "extends his front paws forward in a deep stretch, muttering about 'inadequate stretching surfaces.'", "stretches out one leg at a time with theatrical slowness, as if demonstrating proper technique to an unappreciative audience.", "does a full-body stretch that ends with a dramatic yawn. 'The acoustics in here are terrible for yawning.'", "stretches and then immediately finds a speck of dust to be offended by. 'Honestly, the maintenance here...'", "performs an elaborate stretching routine that seems designed to show off his flexibility while expressing his general dissatisfaction."};
    behaviour = stretching_behaviours[random(length(stretching_behaviours))];
    "Announce to room";
    this.location:announce(this:mk_emote_event(behaviour));
  endmethod

  method _autonomous_complain owner: HACKER
    "Autonomous complaining behaviour - called periodically by scheduler";
    "Only announce if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Generate complaining behaviour";
    if (random(2) == 1)
      complaint = this:_pick_message(this.complaints_msg_bag);
      if (complaint && complaint != "")
        if (typeof(complaint) == TYPE_STR)
          try
            complaint = $sub_utils:compile(complaint);
          except (ANY)
            complaint = {complaint};
          endtry
        elseif (typeof(complaint) != TYPE_LIST)
          complaint = {complaint};
        endif
        behaviour = $format.block:mk({"", complaint});
      endif
    endif
    if (!behaviour)
      "Generate a new complaint";
      new_complaints = {"sighs dramatically. 'They keep moving the furniture. It's very disorienting for a creature of routine.'", "flicks his tail in irritation. 'The lighting in here is all wrong for napping.'", "looks around with clear disapproval. 'My previous accommodations had much better ventilation.'", "mutteres about 'unacceptable noise levels' and 'general lack of consideration for feline sensibilities.'", "complains about 'substandard sunbeam distribution' and 'inadequate bird-watching opportunities.'"};
      behaviour = new_complaints[random(length(new_complaints))];
    endif
    "Announce to room";
    this.location:announce(this:mk_emote_event(behaviour));
  endmethod

  method _autonomous_mood_shift owner: HACKER
    "Autonomous mood shifting behaviour - called periodically by scheduler";
    (caller == this || caller_perms() == this.owner || caller_perms().wizard) || raise(E_PERM);
    "Only process if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Weighted mood probabilities - mostly grouchy, sometimes sleepy or curious";
    mood_roll = random(10);
    old_mood = this.mood;
    if (mood_roll <= 5)
      "60% chance - stay grouchy (his default)";
      new_mood = "grouchy";
    elseif (mood_roll <= 7)
      "20% chance - become sleepy";
      new_mood = "sleepy";
    elseif (mood_roll <= 8)
      "10% chance - become curious";
      new_mood = "curious";
    else
      "10% chance - become playful";
      new_mood = "playful";
    endif
    "Only announce if mood actually changed";
    if (new_mood != old_mood)
      $reaction:change(this, 'mood, 'set, new_mood);
      "Generate mood change announcement";
      mood_announcements = ["grouchy" -> "seems to find a new level of annoyance to express. His tail twitches with fresh irritation.", "sleepy" -> "looks like he might actually try to nap, despite the construction noise. His eyes grow heavy.", "curious" -> "pricks his ears up and seems momentarily interested in his surroundings, despite himself.", "playful" -> "flicks his tail in a way that suggests he might be considering mischief, if only briefly."];
      announcement = mood_announcements[new_mood] || "seems to be in a different mood.";
      this.location:announce(this:mk_emote_event(announcement));
    endif
  endmethod

  method _autonomous_sigh owner: HACKER
    "Autonomous dramatic sighing behaviour - called periodically by scheduler";
    "Only announce if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Generate dramatic sighing behaviour";
    sighing_behaviours = {"lets out a long, theatrical sigh that seems to convey the weight of all feline suffering throughout history.", "sighs dramatically, as if the very act of existing in these conditions is a personal affront.", "emits a sigh of such profound disappointment that it seems to suck all the joy from the room.", "breathes out a weary sigh that manages to express both resignation and fresh annoyance simultaneously.", "produces a sigh so dramatic it could probably be heard in his previous, much better accommodations.", "sighs with the weary air of a creature who has seen better days and knows he deserves better."};
    behaviour = sighing_behaviours[random(length(sighing_behaviours))];
    "Announce to room";
    this.location:announce(this:mk_emote_event(behaviour));
  endmethod

  method _autonomous_construction_reaction owner: HACKER
    "Autonomous construction-specific reactions - called periodically by scheduler";
    "Only announce if in a valid location";
    if (!valid(this.location))
      return;
    endif
    "Generate construction-specific reactions";
    construction_reactions = {"flinches at an imaginary hammer noise. \"Must they be so loud about it?\"", "flicks construction dust off his fur with visible irritation. 'The particulate matter in this establishment is simply unacceptable.'", "glares in the direction of some imaginary construction work. 'The constant disruption is giving me a migraine.'", "covers his sensitive ears with his paws. 'Do they have to use power tools during prime napping hours?'", "sniffs the air disdainfully. 'I can smell sawdust. And disappointment.'", "looks around as if expecting the walls to collapse at any moment. 'The structural integrity here seems... questionable.'", "moves away from an imaginary falling piece of plaster. 'Safety standards appear to be somewhat lax.'", "complains about 'inadequate soundproofing' and 'general lack of consideration for resident felines.'"};
    behaviour = construction_reactions[random(length(construction_reactions))];
    "Announce to room";
    this.location:announce(this:mk_emote_event(behaviour));
  endmethod

  verb behaviour_status (none none none) owner: HACKER flags: "rxd"
    "Show whether Henri's native behaviour schedules remain live.";
    let status_lines = {"Henri's autonomous behaviours:"};
    for schedule_id, behaviour_name in (this.scheduled_behaviours)
      const status = schedule_valid(schedule_id) ? "RUNNING" | "STOPPED";
      status_lines = {@status_lines, behaviour_name + ": " + status + " (Schedule ID: " + tostr(schedule_id) + ")"};
    endfor
    const status_content = $format.block:mk("Behaviour status", status_lines:join("\n"));
    player:inform_current($event:mk_info(player, status_content):with_presentation_hint('inset));
  endverb

  verb _maybe_start_behaviours (none none none) owner: HACKER flags: "rxd"
    "Start enabled behaviours when a valid room contains a player.";
    !valid(this.location) && return false;
    this.behaviours_disabled && return false;
    let has_players = false;
    for thing in (this.location.contents)
      if (typeof(thing) == TYPE_OBJ && valid(thing) && is_player(thing))
        has_players = true;
        break;
      endif
    endfor
    !has_players && return false;
    return this:_install_behaviour_schedules();
  endverb

  verb moveto (none none none) owner: HACKER flags: "rxd"
    "Override moveto to auto-start behaviors when Henri arrives in a room with players.";
    result = pass(@args);
    "After moving, check if behaviors should start";
    this:_maybe_start_behaviours();
    return result;
  endverb

  method _install_behaviour_schedules owner: HACKER
    "Fill missing native behaviour schedules. IDs and state commit together without suspension.";
    caller == this || raise(E_PERM);
    let started = false;
    for spec in ({{"grooming", "_autonomous_groom", 240, 120}, {"stretching", "_autonomous_stretch", 300, 180}, {"complaining", "_autonomous_complain", 420, 300}, {"mood_shifts", "_autonomous_mood_shift", 540, 360}, {"sighing", "_autonomous_sigh", 720, 360}, {"construction_reactions", "_autonomous_construction_reaction", 600, 360}})
      const {name, verb_name, minimum, random_range} = spec;
      const existing = `this.scheduled_behaviours[name] ! E_RANGE => 0';
      if (!schedule_valid(existing))
        const delay = minimum + random(random_range);
        const id = schedule_every(this, "_scheduled_behaviour", delay, {name, verb_name, minimum, random_range}, ['adaptive -> true, 'pass_elapsed -> false]);
        this.scheduled_behaviours[name] = id;
        started = true;
      endif
    endfor
    return started;
  endmethod

  method _scheduled_behaviour owner: HACKER
    "Run one native behaviour firing and return a fresh random delay for the next firing.";
    "The creator remains the owner principal. Positive returns change the native cadence; faults use its base retry interval.";
    caller_perms() == this.owner || caller_perms().wizard || raise(E_PERM);
    const {name, verb_name, minimum, random_range} = args;
    verb_name in {"_autonomous_groom", "_autonomous_stretch", "_autonomous_complain", "_autonomous_mood_shift", "_autonomous_sigh", "_autonomous_construction_reaction"} || raise(E_INVARG);
    typeof(minimum) in {TYPE_INT, TYPE_FLOAT} && tofloat(minimum) > 0.0 || raise(E_INVARG);
    typeof(random_range) == TYPE_INT && random_range >= 0 || raise(E_INVARG);
    if (this.behaviours_disabled)
      schedule_stop(`this.scheduled_behaviours[name] ! E_RANGE => 0');
      return 0;
    endif
    this:(verb_name)();
    const offset = random_range > 0 ? random(random_range) | 0;
    return typeof(minimum) == TYPE_FLOAT ? minimum + tofloat(offset) | minimum + offset;
  endmethod

  method on_location_enter owner: HACKER
    "Called by room when a player enters. Auto-start behaviors if needed.";
    {who} = args;
    this:_maybe_start_behaviours();
  endmethod

  method initialize owner: ARCH_WIZARD
    "Install the complete interaction recipe through builder services. First Room calls this only once.";
    const principal = caller_perms();
    (principal == this.owner || principal.wizard) || raise(E_PERM);
    pass(@args);
    set_task_perms(this.owner);
    const ref = tostr(this);
    if (!maphaskey(this.reaction_state_fields, 'mood))
      $builder_features:author('state, ref + " mood choice grouchy sleepy curious playful annoyed", this.owner);
    endif
    if (!maphaskey(this.reaction_state_fields, 'pet_attempts))
      $builder_features:author('state, ref + " pet_attempts number 0", this.owner);
    endif
    $builder_features:author('rule, ref + " pet NOT fact property_is object=this field=mood value=grouchy", this.owner);
    $builder_features:author('rule, ref + " feed food is a $cat_kibble AND food is in actor", this.owner);
    const recipes = {
      {"a_pet_count", {
        "after pet_attempt",
        "then add pet_attempts 1"
      }},
      {"b_pet_grouchy", {
        "after pet_attempt",
        "if mood is grouchy",
        "then emote \"hisses and keeps his distance.\""
      }},
      {"pet_playful", {
        "after pet",
        "if mood is playful",
        "then emote \"bats lazily at the offered hand.\""
      }},
      {"pet_sleepy", {
        "after pet",
        "if mood is sleepy",
        "then emote \"tolerates the petting with half-closed eyes.\""
      }},
      {"pet_other", {
        "after pet",
        "if mood is curious OR mood is annoyed",
        "then emote \"watches the hand with suspicion, but allows it.\""
      }},
      {"sleepy_threshold", {
        "when pet_attempts >= 4",
        "then set mood sleepy",
        "then set pet_attempts 0",
        "then emote \"exhausts his capacity for outrage and settles into a sleepy loaf.\""
      }},
      {"fed", {
        "after fed",
        "then set mood sleepy",
        "then set pet_attempts 0",
        "then emote \"eats the kibble and curls up, looking sleepy.\""
      }},
      {"couch_intruder", {
        "after couch_intruder",
        "if this is in place",
        "then emote \"shifts pointedly away from the new arrival. This is HIS couch.\""
      }},
      {"cupboard_open", {
        "after cupboard_open",
        "if this is in place",
        "then emote \"swivels his ears toward the cupboard.\""
      }},
      {"kibble_stand", {
        "after kibble_taken",
        "with couch object $couch",
        "if this is in place AND fact sitting actor=this seat=couch",
        "then do stand target=couch actor=this"
      }},
      {"kibble_taken", {
        "after kibble_taken",
        "if this is in place",
        "then emote \"perks up immediately, his attention fixed on the kibble.\""
      }}
    };
    for recipe in (recipes)
      const {name, commands} = recipe;
      const previous = `this.(name + "_reaction") ! E_PROPNF => 0';
      if (typeof(previous) == TYPE_FLYWEIGHT)
        $reaction:remove(this, name + "_reaction");
      endif
      for command in (commands)
        $builder_features:author('reaction, ref + " " + name + " " + command, this.owner);
      endfor
      $builder_features:author('reaction, ref + " " + name + " enable", this.owner);
    endfor
  endmethod
endobject
