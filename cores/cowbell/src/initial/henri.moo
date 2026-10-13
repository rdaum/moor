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
  property couch_intruder_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " shifts pointedly away from ",
    <SUB, .capitalize = false, .type = 'actor>,
    ", radiating offended dignity. \"This is MY couch.\""
  };
  property couch_intruder_reaction (owner: HACKER, flags: "r") = 0;
  property cupboard_open_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    "'s ears swivel like radar dishes toward the cupboard."
  };
  property cupboard_open_reaction (owner: HACKER, flags: "r") = 0;
  property feed_denied_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'actor>,
    " turns ",
    <SUB, .capitalize = false, .type = 'pos_adj>,
    " nose up at the offering with disdain."
  };
  property feed_rule (owner: HACKER, flags: "r") = <RULE, .name = 'henri_feed_rule, .body = {
      {'is_grouchy, {'var, 'This}},
      {'isa, {'var, 'Food}, CAT_KIBBLE},
      {'location_is, {'var, 'Food}, {'var, 'Accessor}}
    }, .variables = {'This, 'Food, 'Accessor}, .head = 'henri_feed_rule>;
  property kibble_taken_msg_bag (owner: HACKER, flags: "r") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'dobj>,
      " starts weaving around ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " legs, purring loudly."
    }, {
      <SUB, .capitalize = true, .type = 'dobj>,
      " perks up immediately, tail suddenly upright and alert. The sulking can wait."
    }, {
      <SUB, .capitalize = true, .type = 'dobj>,
      " trots over with uncharacteristic enthusiasm. \"Well, finally some proper service.\""
    }, {
      <SUB, .capitalize = true, .type = 'dobj>,
      " abandons ",
      <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
      " aloof demeanor entirely, eyes locked on the kibble."
    }, {
      <SUB, .capitalize = true, .type = 'dobj>,
      " forgets to be grouchy for a moment, whiskers twitching with anticipation."
    }}>;
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
  property mood (owner: HACKER, flags: "rc") = "sleepy";
  property on_pet_grouchy_reaction (owner: HACKER, flags: "r") = 0;
  property on_pet_playful_reaction (owner: HACKER, flags: "r") = 0;
  property pet_denied_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {"{nc} hisses and swats at {p_dobj} hand before {dobj} can touch him.", "{nc} lets out a low growl and bares {p} teeth at {dobj}.", "{nc} backs away with a suspicious glare, clearly warning {dobj} to keep {p} distance.", "{nc} flexes {p} claws menacingly as {dobj} approaches, a silent but clear threat."}>;
  property pet_grouchy_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " flattens ",
    <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
    " ears against ",
    <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
    " skull."
  };
  property pet_playful_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " bats lazily at ",
    <SUB, .capitalize = false, .type = 'pos_adj>,
    " hand."
  };
  property pet_reactions_default_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'actor>,
      " sighs but doesn't pull away immediately. \"I suppose I'm getting used to this indignity.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " tolerates the petting with weary resignation. \"At least ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " technique has improved slightly.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " leans into the petting for a moment before remembering ",
      <SUB, .type = 'subject>,
      " is supposed to be annoyed."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " gives you a look that says \"I'm only allowing this because the alternative is listening to more hammering.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " tolerates ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " attention with the weary resignation of a cat who has seen better days and better accommodations."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " flicks an ear in ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " direction. \"If ",
      <SUB, .capitalize = false, .type = 'self_alt, .for_self = "you're", .for_others = "they're">,
      " quite done disrupting my brooding session...\""
    }}>;
  property pet_reactions_grouchy_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'actor>,
      " flattens ",
      <SUB, .type = 'pos_adj>,
      " ears and gives you a look of pure betrayal. \"I was in the middle of a very important sulk,\" ",
      <SUB, .type = 'subject>,
      " seems to say."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " allows exactly two pets before pulling away with an offended sniff. \"That's quite enough of that,\" ",
      <SUB, .type = 'pos_adj>,
      " posture suggests."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " tolerates the petting for precisely three seconds before ",
      <SUB, .type = 'pos_adj>,
      " tail starts twitching violently. \"Are you quite finished?\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " turns ",
      <SUB, .capitalize = false, .type = 'pos_adj>,
      " head away dramatically. \"I'm not some common housecat to be petted at ",
      <SUB, .capitalize = false, .type = 'self_alt, .for_self = "your", .for_others = "their">,
      " whim. I have standards, you know.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " lets out a long-suffering sigh. \"Fine, if you must. But don't expect me to enjoy it.\""
    }}>;
  property pet_reactions_sleepy_msg_bag (owner: HACKER, flags: "rc") = <MSG_BAG, {{
      <SUB, .capitalize = true, .type = 'actor>,
      " purrs reluctantly, as if doing you a great favor. \"I suppose this is acceptable. For now.\""
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " leans into the petting for a moment before remembering ",
      <SUB, .type = 'subject>,
      " is supposed to be annoyed. ",
      <SUB, .type = 'subject>,
      " pulls away with a huff."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " closes ",
      <SUB, .type = 'pos_adj>,
      " eyes and pretends to enjoy it, though you can tell ",
      <SUB, .type = 'subject>,
      " ",
      <SUB, .type = 'verb_be>,
      " just too tired to protest properly."
    }, {
      <SUB, .capitalize = true, .type = 'actor>,
      " murmurs something that sounds like \"The vibrations are almost as good as the old heating vent... almost.\""
    }}>;
  property pet_rule (owner: HACKER, flags: "r") = <RULE, .name = 'henri_pet_rule, .body = {{'not, {'is_grouchy, {'var, 'This}}}}, .variables = {'This}, .head = 'henri_pet_rule>;
  property pets_received (owner: HACKER, flags: "rc") = 33;
  property scheduled_behaviours (owner: HACKER, flags: "rc") = [];
  property sleepy_threshold_msg (owner: HACKER, flags: "r") = {
    <SUB, .capitalize = true, .type = 'dobj>,
    " seems to have exhausted ",
    <SUB, .capitalize = false, .type = 'dobj_pos_adj>,
    " capacity for outrage and slumps into a resigned loaf."
  };
  property sleepy_threshold_reaction (owner: HACKER, flags: "r") = 0;

  override aliases (owner: HACKER, flags: "rc") = {"cat", "grouchy cat"};
  override description (owner: HACKER, flags: "rc") = "A sleek black cat with piercing green eyes and an air of perpetual annoyance. His fur is immaculately groomed despite the construction dust, and he holds himself with the offended dignity of a creature who knows he deserves better accommodations. He occasionally flicks his tail in irritation, as if to emphasize his displeasure with the current state of affairs.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Henri - The Grouchy Cat",
    "",
    "## Overview",
    "",
    "Henri is a grouchy black cat NPC with autonomous behaviors and a rule-based interaction system.",
    "He has moods, tracks interaction history, and uses the rule engine to determine who can interact with him.",
    "",
    "## Interaction Rules",
    "",
    "Henri's behavior is controlled by rules that can be customized:",
    "",
    "- `pet_rule`: Controls who can pet Henri (default: `0` = anyone can try)",
    "- `feed_rule`: Controls who can feed Henri (default: `0` = anyone can try)",
    "",
    "### Example Rules",
    "",
    "Only allow petting when Henri is NOT grouchy:",
    "```",
    "@set-rule henri.pet_rule NOT This is_grouchy?",
    "```",
    "",
    "Only allow petting when Henri is sleepy OR the accessor has earned tolerance:",
    "```",
    "@set-rule henri.pet_rule This is_sleepy? OR This tolerates(Accessor)?",
    "```",
    "",
    "Only allow feeding when Henri is grouchy (he's too proud to eat otherwise):",
    "```",
    "@set-rule henri.feed_rule This is_grouchy?",
    "```",
    "",
    "## Available Fact Predicates",
    "",
    "Henri provides these predicates for use in rules:",
    "",
    "- `This is_grouchy?` - Returns true if Henri's mood is \"grouchy\"",
    "- `This is_sleepy?` - Returns true if Henri's mood is \"sleepy\"",
    "- `This is_curious?` - Returns true if Henri's mood is \"curious\"",
    "- `This is_playful?` - Returns true if Henri's mood is \"playful\"",
    "- `This tolerates(Accessor)?` - Returns true if accessor has petted Henri more than 10 times",
    "",
    "## Properties",
    "",
    "- `mood`: Current mood state (\"grouchy\", \"sleepy\", \"curious\", \"playful\", \"annoyed\")",
    "- `pets_received`: Counter tracking total times Henri has been petted",
    "- `scheduled_behaviours`: Map of active autonomous behavior tasks",
    "",
    "## Commands",
    "",
    "- `pet henri` / `stroke henri` - Attempt to pet Henri (subject to pet_rule)",
    "- `feed henri` / `give henri` - Attempt to feed Henri (subject to feed_rule)",
    "- `henri:start_behaviours()` - Start Henri's autonomous behaviors",
    "- `henri:stop_behaviours()` - Stop Henri's autonomous behaviors",
    "- `henri:change_mood()` - Manually cycle Henri's mood for testing",
    "- `henri:behaviour_status()` - Show status of autonomous behaviors",
    "",
    "## Autonomous Behaviors",
    "",
    "When started, Henri performs these behaviors periodically:",
    "",
    "- Grooming (every 4-6 minutes)",
    "- Stretching (every 5-8 minutes)",
    "- Complaining (every 7-12 minutes)",
    "- Mood shifts (every 9-15 minutes)",
    "- Dramatic sighs (every 12-18 minutes)",
    "- Construction reactions (every 10-16 minutes)"
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

  method fact_is_grouchy owner: HACKER
    "Rule predicate: Is Henri in a grouchy mood?";
    {henri} = args;
    return henri.mood == "grouchy";
  endmethod

  method fact_is_sleepy owner: HACKER
    "Rule predicate: Is Henri in a sleepy mood?";
    {henri} = args;
    return henri.mood == "sleepy";
  endmethod

  method fact_is_curious owner: HACKER
    "Rule predicate: Is Henri in a curious mood?";
    {henri} = args;
    return henri.mood == "curious";
  endmethod

  method fact_is_playful owner: HACKER
    "Rule predicate: Is Henri in a playful mood?";
    {henri} = args;
    return henri.mood == "playful";
  endmethod

  method fact_tolerates owner: HACKER
    "Rule predicate: Has accessor petted Henri enough to be tolerated?";
    {henri, accessor} = args;
    return henri.pets_received > 10;
  endmethod

  method can_be_petted owner: HACKER
    "Check if accessor can pet Henri. Returns {allowed, reason}.";
    {accessor} = args;
    "No rule = anyone can try to pet";
    if (this.pet_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate pet rule";
    result = $rule_engine:evaluate(this.pet_rule, ['This -> this, 'Accessor -> accessor]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      return ['allowed -> false, 'reason -> this:_pick_message(this.pet_denied_msg_bag)];
    endif
  endmethod

  method can_be_fed owner: HACKER
    "Check if accessor can feed Henri with food item. Returns {allowed, reason}.";
    {accessor, food} = args;
    "No rule = anyone can feed";
    if (this.feed_rule == 0)
      return ['allowed -> true, 'reason -> {}];
    endif
    "Evaluate feed rule";
    result = $rule_engine:evaluate(this.feed_rule, ['This -> this, 'Accessor -> accessor, 'Food -> food]);
    if (result['success])
      return ['allowed -> true, 'reason -> {}];
    else
      return ['allowed -> false, 'reason -> this.feed_denied_msg];
    endif
  endmethod

  verb "pet stroke" (this none none) owner: HACKER flags: "rxd"
    "Handle petting Henri - uses rule system to determine if allowed";
    "Increment pets counter (attempts count towards his resignation)";
    $reaction:change(this, 'pets_received, 'increment, 1, ['Actor -> player]);
    "Check access via rule";
    access_check = this:can_be_petted(player);
    if (!access_check['allowed])
      "Henri doesn't allow this petting attempt";
      if (valid(this.location))
        reaction_content = access_check['reason];
        if (typeof(reaction_content) == TYPE_STR)
          try
            reaction_content = $sub_utils:compile(reaction_content);
          except (ANY)
            reaction_content = {reaction_content};
          endtry
        elseif (typeof(reaction_content) != TYPE_LIST)
          reaction_content = {reaction_content};
        endif
        event = $event:mk_emote(this, @reaction_content):with_dobj(player):with_audience('narrative);
        this.location:announce(event);
      endif
      return;
    endif
    "Announce the action to the room";
    if (valid(this.location))
      this.location:announce($event:mk_emote(player, $sub:nc(), " ", $sub:self_alt("pet", "pets"), " ", $sub:the('dobj), "."):with_dobj(this));
    endif
    "Fire trigger for reactions";
    this:fire_trigger('on_pet, ['Actor -> player]);
    "Determine Henri's reaction based on mood, random chance, and interaction history";
    reaction = "";
    "Check if he's becoming slightly more tolerant over time";
    if (this.pets_received > 10 && random(3) == 1)
      "He's been petted a lot - might be slightly more tolerant";
      reaction = this:_pick_message(this.pet_reactions_default_msg_bag);
    elseif (this.mood == "grouchy")
      "Grouchy mood - mostly negative reactions";
      reaction = this:_pick_message(this.pet_reactions_grouchy_msg_bag);
    elseif (this.mood == "sleepy")
      "Sleepy mood - more tolerant but still grumpy";
      reaction = this:_pick_message(this.pet_reactions_sleepy_msg_bag);
    else
      "Default reaction";
      reaction = this:_pick_message(this.pet_reactions_default_msg_bag);
    endif
    "Occasionally mix in a complaint about the construction";
    if (random(3) == 1)
      complaint = this:_pick_message(this.complaints_msg_bag);
      if (complaint && complaint != "")
        "Ensure both reaction and complaint are lists before combining";
        reaction_content = reaction;
        if (typeof(reaction_content) == TYPE_STR)
          try
            reaction_content = $sub_utils:compile(reaction_content);
          except (ANY)
            reaction_content = {reaction_content};
          endtry
        elseif (typeof(reaction_content) != TYPE_LIST)
          reaction_content = {reaction_content};
        endif
        complaint_content = complaint;
        if (typeof(complaint_content) == TYPE_STR)
          try
            complaint_content = $sub_utils:compile(complaint_content);
          except (ANY)
            complaint_content = {complaint_content};
          endtry
        elseif (typeof(complaint_content) != TYPE_LIST)
          complaint_content = {complaint_content};
        endif
        reaction = $format.block:mk(reaction_content, complaint_content);
      endif
    endif
    "Announce Henri's reaction";
    if (valid(this.location))
      reaction_content = reaction;
      if (typeof(reaction_content) == TYPE_STR)
        try
          reaction_content = $sub_utils:compile(reaction_content);
        except (ANY)
          reaction_content = {reaction_content};
        endtry
      elseif (typeof(reaction_content) != TYPE_LIST)
        reaction_content = {reaction_content};
      endif
      event = $event:mk_emote(this, @reaction_content):with_dobj(player):with_audience('narrative);
      this.location:announce(event);
    endif
  endverb

  verb change_mood (none none none) owner: HACKER flags: "rxd"
    "Change Henri's mood state for testing";
    "Available moods";
    moods = {"grouchy", "sleepy", "curious", "playful", "annoyed"};
    "Cycle to next mood";
    current_index = this.mood in moods;
    if (current_index == 0)
      new_mood = "grouchy";
    else
      new_index = current_index % length(moods) + 1;
      new_mood = moods[new_index];
    endif
    "Update mood";
    old_mood = this.mood;
    this.mood = new_mood;
    "Provide feedback";
    player:inform_current($event:mk_info(player, "Henri's mood changed from " + old_mood + " to " + new_mood + "."));
    "Announce mood change if in a room";
    if (valid(this.location))
      mood_descriptions = ["grouchy" -> "seems to find a new level of annoyance to express.", "sleepy" -> "looks like he might actually try to nap, despite the construction noise.", "curious" -> "pricks his ears up and seems momentarily interested in his surroundings.", "playful" -> "flicks his tail in a way that suggests he might be considering mischief.", "annoyed" -> "lets out a sigh that conveys profound disappointment with the universe."];
      description = mood_descriptions[new_mood] || "seems to be in a mood.";
      this.location:announce(this:mk_emote_event(description));
    endif
  endverb

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

  verb "feed give" (this with any) owner: HACKER flags: "rxd"
    "Handle feeding Henri - uses rule system to determine if allowed";
    "Match the food item from player's inventory";
    if (!iobjstr || iobjstr == "")
      event = $event:mk_error(player, "Feed ", $sub:d(), " what?"):with_dobj(this);
      player:inform_current(event);
      return;
    endif
    try
      food = $match:match_object(iobjstr, player);
    except e (ANY)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endtry
    if (!valid(food) || typeof(food) != TYPE_OBJ)
      event = $event:mk_error(player, "You don't have that.");
      player:inform_current(event);
      return;
    endif
    if (food.location != player)
      event = $event:mk_error(player, "You need to be holding ", $sub:i(), " to feed it to ", $sub:d(), "."):with_dobj(this):with_iobj(food);
      player:inform_current(event);
      return;
    endif
    "Check access via rule";
    access_check = this:can_be_fed(player, food);
    if (!access_check['allowed])
      "Henri doesn't accept this food offering";
      if (valid(this.location))
        reaction_content = access_check['reason];
        if (typeof(reaction_content) == TYPE_STR)
          try
            reaction_content = $sub_utils:compile(reaction_content);
          except (ANY)
            reaction_content = {reaction_content};
          endtry
        elseif (typeof(reaction_content) != TYPE_LIST)
          reaction_content = {reaction_content};
        endif
        event = $event:mk_emote(this, @reaction_content):with_dobj(player):with_audience('narrative);
        this.location:announce(event);
      endif
      return;
    endif
    "Announce the action to the room";
    if (valid(this.location))
      this.location:announce($event:mk_emote(player, $sub:nc(), " ", $sub:self_alt("offer", "offers"), " ", $sub:i(), " to ", $sub:the('dobj), "."):with_dobj(this):with_iobj(food));
    endif
    "Consume the food - move it to Henri (eaten)";
    old_mood = this.mood;
    food:moveto(this);
    "Fire trigger for reactions";
    this:fire_trigger('on_fed, ['Actor -> player, 'Food -> food]);
    "Improve mood after eating - cats get sleepy/content after food";
    mood_improvements = ["grouchy" -> "sleepy", "annoyed" -> "sleepy", "curious" -> "playful", "playful" -> "playful", "sleepy" -> "sleepy"];
    if (maphaskey(mood_improvements, this.mood))
      this.mood = mood_improvements[this.mood];
    endif
    "Generate Henri's picky response based on whether mood improved";
    mood_changed = old_mood != this.mood;
    if (mood_changed)
      "Mood improved - slightly less grumpy responses";
      responses = {"sniffs delicately, then deigns to eat. After a moment, his eyes droop slightly. \"Acceptable. I suppose I could use a nap.\"", "eats with grudging acceptance, then settles into a more relaxed posture. \"Don't think this changes anything between us.\"", "consumes the offering methodically. A subtle rumble of purring escapes before he catches himself. \"That was... adequate.\"", "finishes eating and begins grooming his whiskers with slightly less irritation than usual. \"The presentation was all wrong, but the quality was... tolerable.\"", "eats, then curls his tail around his paws - a rare sign of contentment. \"I'll allow that this wasn't entirely terrible.\""};
    else
      "Mood didn't change - standard picky responses";
      responses = {"takes a tentative bite, then gives you a look that says \"It's adequate. Barely.\"", "looks at the food, then at you, then back at the food. \"My previous establishment served this with a proper garnish.\"", "picks at the food disdainfully. \"The texture is all wrong. And the temperature is completely off.\"", "eats quickly, as if embarrassed to be seen accepting such pedestrian fare. \"This better not become a habit.\""};
    endif
    response = responses[random(length(responses))];
    "Announce Henri's reaction";
    if (valid(this.location))
      this.location:announce(this:mk_emote_event(response));
    endif
    "Announce mood shift if it happened";
    if (mood_changed && valid(this.location))
      mood_descriptions = ["sleepy" -> "eyes are getting heavy - apparently that meal was satisfactory enough to induce drowsiness."];
      if (maphaskey(mood_descriptions, this.mood))
        event = $event:mk_social(this, $sub:nc(), "'s ", mood_descriptions[this.mood]):with_this(this.location);
        this.location:announce(event);
      endif
    endif
  endverb

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
      this.mood = new_mood;
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
    "Install this object's reactions through the same owner-authorized API used by builders.";
    const principal = caller_perms();
    principal == this.owner || principal.wizard || raise(E_PERM);
    set_task_perms(principal);
    pass(@args);
    $reaction:install(this, "couch_intruder_reaction", <$reaction, .enabled = true, .trigger = 'on_couch_intruder, .when = 0, .effects = {{'announce, 'couch_intruder_msg}}, .fired_at = 0>);
    $reaction:install(this, "cupboard_open_reaction", <$reaction, .enabled = true, .trigger = 'on_cupboard_open, .when = 0, .effects = {{'announce, 'cupboard_open_msg}}, .fired_at = 0>);
    $reaction:install(this, "kibble_taken_reaction", <$reaction, .enabled = true, .trigger = 'on_kibble_taken, .when = 0, .effects = {{'action, 'stand, $couch}, {'announce, 'kibble_taken_msg_bag}}, .fired_at = 0>);
    $reaction:install(this, "on_pet_grouchy_reaction", <$reaction, .enabled = true, .trigger = 'on_pet, .when = <$rule, .name = 'grouchy_rule, .body = {{'is_grouchy, {'var, 'This}}}, .variables = {'This}, .head = 'grouchy_rule>, .effects = {{'announce, 'pet_grouchy_msg}}, .fired_at = 0>);
    $reaction:install(this, "on_pet_playful_reaction", <$reaction, .enabled = true, .trigger = 'on_pet, .when = <$rule, .name = 'playful_rule, .body = {{'is_playful, {'var, 'This}}}, .variables = {'This}, .head = 'playful_rule>, .effects = {{'announce, 'pet_playful_msg}}, .fired_at = 0>);
    $reaction:install(this, "sleepy_threshold_reaction", <$reaction, .enabled = true, .trigger = {'when, 'pets_received, 'ge, 4}, .when = 0, .effects = {
      {'set, 'mood, "sleepy"},
      {'announce, 'sleepy_threshold_msg},
      {'action, 'sit, $couch}
    }, .fired_at = 0>);
  endmethod
endobject
