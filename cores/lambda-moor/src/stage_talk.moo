object STAGE_TALK
  name: "Stage-Talk Feature"
  parent: FEATURE
  owner: HACKER
  fertile: true
  readable: true

  override aliases = {"Stage-Talk Feature"};
  override description = {
    "This feature contains various verbs used in stage talk, which allows players to describe their actions in terms of stage directions instead of prose."
  };
  override feature_verbs = {"`", "[", "]", "-", "<"};
  override help_msg = {
    "This feature contains various verbs used in stage talk, which allows players to describe their actions in terms of stage directions instead of prose."
  };
  override import_export_id = "stage_talk";
  override object_size = {4109, 1084848672};

  verb "stage `* -*" (any any any) owner: HACKER flags: "rxd"
    "Say something out loud, directed at someone or something.";
    "Usage:";
    "  `target message";
    "Example:";
    "  Munchkin is talking to Kenneth, who's in the same room with him.  He types:";
    "      `kenneth What is the frequency?";
    "  The room sees:";
    "       Munchkin [to Kenneth]: What is the frequency?";
    name = verb[2..$];
    who = player.location:match_object(name);
    if ($command_utils:object_match_failed(who, name))
      return;
    endif
    player.location:announce_all(player.name, " [to ", who.name, "]: ", argstr);
  endverb

  verb "stage [*" (any any any) owner: HACKER flags: "rxd"
    "Say something out loud, in some specific way.";
    "Usage:";
    "  [how]: message";
    "Example:";
    "  Munchkin decideds to sing some lyrics.  He types:";
    "      [sings]: I am the eggman";
    "  The room sees:";
    "      Munchkin [sings]: I am the eggman";
    player.location:announce_all(player.name + " " + verb + " " + argstr);
  endverb

  verb "stage ]*" (any any any) owner: HACKER flags: "rxd"
    "Perform some physical, non-verbal, action.";
    "Usage:";
    "  ]third person action";
    "Example:";
    "  Munchkin has annoyed some would-be tough guy.  He types:";
    "      ]hides behind the reactor.";
    "  The room sees:";
    "      [Munchkin hides behind the reactor.]";
    player.location:announce_all("[", player.name + " " + verb[2..$] + (argstr ? " " + argstr | "") + "]");
  endverb

  verb "~*" (any any any) owner: HACKER flags: "rxd"
    name = verb[2..$];
    argstr = $code_utils:argstr(verb, args, argstr);
    player.location:announce_all(player.name, " [", name, "]: ", argstr);
  endverb

  verb "stage <*" (any any any) owner: HACKER flags: "rxd"
    "Point to yourself.";
    "Usage:";
    "  <message";
    "Example:";
    "  Muchkin decides he's being strange. He types:";
    "    <being strange.";
    "  The room sees:";
    "    Munchkin <- being strange.";
    player.location:announce_all(player.name + " <- " + verb[2..$] + " " + argstr);
  endverb
endobject