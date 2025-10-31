object YOU
  name: "you"
  parent: GENDERED_OBJECT
  owner: HACKER
  readable: true

  property conjugations (owner: HACKER, flags: "r") = {{"is", "are"}, {"was", "were"}, {"does", "do"}, {"has", "have"}};
  property help_msg (owner: HACKER, flags: "rc") = {
    "This object is useful for announcing messages that switch between third and second person when addressed to the appropriate parties in a room.",
    "",
    "Verbs:",
    "",
    "  :verb_sub(STR verbspec) -> conjugates the given verb into singular form",
    "  :say_action(message [,who [,thing, [,where]]]) -> appropriately pronoun ",
    "      substituted message announced to where, which defaults to who.location",
    "      where who defaults to player.",
    "  Ex:  if player=#123 (Munchkin), dobj=#456 (Frebblebit), and iobj=#789",
    "       (Bob) and they are all in the same room,",
    "       $you:say_action(\"%N %<waves> happily to %d and %i.\") would do this:",
    "",
    "Munchkin sees:       You wave happily to Frebblebit and Bob.",
    "Frebblebit sees:     Munchkin waves happily to you and Bob.",
    "Bob sees:            Munchkin waves happily to Frebblebit and you.",
    "Everyone else sees:  Munchkin waves happily to Frebblebit and Bob."
  };

  override aliases = {"you"};
  override description = {
    "An object useful for pronoun substitution for switching between third and second person.  See `help $you' for details."
  };
  override gender = "2nd";
  override import_export_id = "you";
  override object_size = {4737, 1084848672};
  override po = "you";
  override poc = "You";
  override pp = "your";
  override ppc = "Your";
  override pq = "yours";
  override pqc = "Yours";
  override pr = "yourself";
  override prc = "Yourself";
  override ps = "you";
  override psc = "You";

  verb verb_sub (this none this) owner: HACKER flags: "rxd"
    "$you:verb_sub(STR verbspec) -> returns verbspec conjugated for singular use as if `you' were saying it.";
    return $gender_utils:get_conj(args[1], this);
    x = args[1];
    len = length(x);
    if (len > 3 && rindex(x, "n't") == len - 3)
      return this:verb_sub(x[1..len - 3]) + "n't";
    endif
    for y in (this.conjugations)
      if (x == y[1])
        return y[2];
      endif
    endfor
    for y in ({{"ches", "ch"}, {"ies", "y"}, {"sses", "ss"}, {"shes", "sh"}, {"s", ""}})
      if (len > length(y[1]) && rindex(x, y[1]) == len - length(y[1]) + 1)
        return x[1..len - length(y[1])] + y[2];
      endif
    endfor
    return x;
  endverb

  verb say_action (this none this) owner: HACKER flags: "rx"
    "$you:say_action(message [,who [,thing, [,where [, excluding-whom]]]])";
    "announce 'message' with pronoun substitution as if it were just ";
    "  where:announce_all_but(excluding-whom, ";
    "    $string_utils:pronoun_sub(message, who, thing, where));";
    "except that who (player), dobj, and iobj get modified messages, with the appropriate use of 'you' instead of their name, and except that `excluding-whom' isn't really a valid variable name.";
    "who       default player";
    "thing     default object that called this verb";
    "where     default who.location";
    "excluding default {}";
    {msg, ?who = player, ?thing = caller, ?where = who.location, ?excluding = {}} = args;
    you = this;
    if (typeof(msg) == LIST)
      tell = "";
      for x in (msg)
        tell = tell + (typeof(x) == STR ? x | x[random(length(x))]);
      endfor
    else
      tell = msg;
    endif
    if (!(who in excluding))
      who:tell($string_utils:pronoun_sub(this:fixpos(tell, "%n"), you, thing, where));
    endif
    if ($object_utils:has_callable_verb(where, "announce_all_but"))
      where:announce_all_but({dobj, who, iobj, @excluding}, $string_utils:pronoun_sub(tell, who, thing, where));
    endif
    if (valid(dobj) && dobj != who && !(dobj in excluding))
      x = dobj;
      dobj = you;
      x:tell($string_utils:pronoun_sub(this:fixpos(tell, "%d"), who, thing, where));
      dobj = x;
    endif
    if (valid(iobj) && !(iobj in {who, dobj, @excluding}))
      x = iobj;
      iobj = you;
      x:tell($string_utils:pronoun_sub(this:fixpos(tell, "%i"), who, thing, where));
      iobj = x;
    endif
  endverb

  verb fixpos (this none this) owner: HACKER flags: "rxd"
    "This is horribly dwimmy.  E.g. %x's gets turned into your, %X's gets turned into Your, and %X'S gets turned into YOUR. --Nosredna";
    upper = $string_utils:uppercase(args[2]);
    allupper = upper + "'S";
    upper = upper + "'s";
    lower = $string_utils:lowercase(args[2]) + "'s";
    return strsub(strsub(strsub(args[1], lower, "your", 1), upper, "Your", 1), allupper, "YOUR", 1);
  endverb

  verb reflexive (this none this) owner: HACKER flags: "rxd"
    "Copied from you (#67923):reflexive [verb author Blob (#21528)] at Wed Jul 13 05:09:32 2005 PDT";
    ":reflexive(msg, %[di])";
    "Make a message reflexive by replacing %d or %i with %r.";
    {msg, pos} = args;
    upper = $string_utils:uppercase(pos) + "'s";
    lower = $string_utils:lowercase(pos) + "'s";
    msg = strsub(msg, lower, "%p", 1);
    msg = strsub(msg, upper, "%P", 1);
    msg = strsub(msg, pos, "%r", 1);
    msg = strsub(msg, $string_utils:uppercase(pos), "%R", 1);
    return msg;
  endverb

  verb say_action_reflexive (this none this) owner: #2 flags: "rx"
    "$you:say_action(message [,who [,thing, [,where [, excluding-whom]]]])";
    "announce 'message' with pronoun substitution as if it were just ";
    "  where:announce_all_but(excluding-whom, ";
    "    $string_utils:pronoun_sub(message, who, thing, where));";
    "except that who (player), dobj, and iobj get modified messages, with the appropriate use of 'you' instead of their name, and except that `excluding-whom' isn't really a valid variable name.";
    "who       default player";
    "thing     default object that called this verb";
    "where     default who.location";
    "excluding default {}";
    {msg, ?who = player, ?thing = caller, ?where = who.location, ?excluding = {}} = args;
    you = this;
    if (typeof(msg) == LIST)
      tell = "";
      for x in (msg)
        tell = tell + (typeof(x) == STR ? x | x[random(length(x))]);
      endfor
    else
      tell = msg;
    endif
    if (who == dobj)
      tell = this:reflexive(tell, "%d");
    endif
    if (who == iobj)
      tell = this:reflexive(tell, "%i");
    endif
    if (!(who in excluding))
      msg = tell;
      x = dobj;
      y = iobj;
      dobj = dobj == who ? you | dobj;
      iobj = iobj == who ? you | iobj;
      who:tell($string_utils:pronoun_sub(this:fixpos(msg, "%n"), you, thing, where));
      dobj = x;
      iobj = y;
    endif
    if ($object_utils:has_callable_verb(where, "announce_all_but"))
      where:announce_all_but({dobj, who, iobj, @excluding}, $string_utils:pronoun_sub(tell, who, thing, where));
    endif
    if (valid(dobj) && dobj != who && !(dobj in excluding))
      x = dobj;
      y = iobj;
      msg = this:fixpos(tell, "%d");
      if (dobj == iobj)
        iobj = you;
        msg = this:fixpos(msg, "%i");
      endif
      dobj = you;
      x:tell($string_utils:pronoun_sub(msg, who, thing, where));
      dobj = x;
      iobj = y;
    endif
    if (valid(iobj) && !(iobj in {who, dobj, @excluding}))
      x = iobj;
      iobj = you;
      x:tell($string_utils:pronoun_sub(this:fixpos(tell, "%i"), who, thing, where));
      iobj = x;
    endif
  endverb
endobject