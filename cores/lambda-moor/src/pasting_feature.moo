object PASTING_FEATURE
  name: "Pasting Feature"
  parent: FEATURE
  owner: HACKER
  fertile: true
  readable: true

  override aliases = {"Pasting Feature"};
  override description = "Verbs useful to people using a windowing system to paste text at people.";
  override feature_verbs = {"@paste", "|", "@paste-to"};
  override help_msg = "The Pasting Feature is mostly useful to people with fancy clients (such as Emacs) or who connect using a windowing system that allows them to copy text they've already seen.  It's intended to give people a way to quote verbatim text at other people in the room.";
  override object_size = {4210, 1084848672};

  verb "@paste" (any any any) owner: HACKER flags: "rx"
    "Usage: @paste <prefix> <suffix>";
    "Announce a series of entered lines to the room the player is in.";
    "Before the lines are quoted, player.paste_header is run through";
    "$string_utils:pronoun_sub(), and if the result contains the player's";
    "name, it is used as a header.  Otherwise player.name centered in a";
    "line of dashes is used.";
    "A footer comes afterwards, likewise derived from player.paste_footer.";
    "<prefix> and <suffix> are placed before and after each line.";
    "";
    "This verb is, as one might guess, designed for pasting text to MOO using";
    "GnuEmacs or a windowing system.  You should remember that after you";
    "have pasted the lines in, you must type . on a line by itself, or you'll";
    "sit around waiting for $command_utils:read_lines() to finish _forever_.";
    {?prefix = "", ?suffix = ""} = args;
    lines = $command_utils:read_lines();
    header = $string_utils:pronoun_sub_secure($code_utils:verb_or_property(player, "paste_header"), "") || $string_utils:center(player.name, 75, "-");
    to_tell = {header};
    for line in (lines)
      to_tell = listappend(to_tell, prefix + line + suffix);
    endfor
    to_tell = listappend(to_tell, $string_utils:pronoun_sub_secure($code_utils:verb_or_property(player, "paste_footer"), "") || $string_utils:center("finished", 75, "-"));
    for thing in (player.location.contents)
      $command_utils:suspend_if_needed(0);
      thing:tell_lines(to_tell);
    endfor
    player:tell("Done @pasting.");
  endverb

  verb "|*" (any any any) owner: HACKER flags: "rxd"
    "Echo a line prefaced by a vertical bar.";
    "Usage:";
    "  |message";
    "Example:";
    "  Hacker wants to echo to the room what he just saw. He enters (either by hand, or with Emacs or a windowing system):";
    "      |Haakon has disconnected.";
    "  The room sees:";
    "      Hacker | Haakon has disconnected.";
    player.location:announce_all(player.name + " | " + verb[2..$] + " " + argstr);
  endverb

  verb "@pasteto @paste-to" (any none none) owner: HACKER flags: "rxd"
    "Syntax: @paste-to <player>";
    "";
    "Which will then prompt you for the lines to privately send to <player>. The lines will be surrounded by a default footer and header.";
    target = $string_utils:match_player(dobjstr);
    $command_utils:player_match_result(target, dobjstr);
    if (!valid(target))
      return;
    endif
    prefix = "";
    suffix = "";
    lines = $command_utils:read_lines();
    to_tell = {$string_utils:center("Private message from " + player.name, 75, "-")};
    for line in (lines)
      to_tell = listappend(to_tell, prefix + line + suffix);
    endfor
    to_tell = listappend(to_tell, $string_utils:center("end message", 75, "-"));
    target:tell_lines(to_tell);
    player:tell("Done @pasting.");
  endverb
endobject