object NEWS
  name: "News"
  parent: MAIL_RECIPIENT
  location: MAIL_AGENT
  owner: HACKER
  readable: true

  property archive_news (owner: HACKER, flags: "rc") = {};
  property current_news (owner: HACKER, flags: "rc") = {1, 2};
  property current_news_going (owner: HACKER, flags: "rc") = {};
  property last_news_time (owner: HACKER, flags: "rc") = 1084848652;

  override aliases = {"News"};
  override description = "It's the current issue of the News, dated %d.";
  override expire_period = 0;
  override last_msg_date = 1084848652;
  override last_used_time = 1084848652;
  override mail_forward = {};
  override messages = {
    {
      1,
      {
        1084848652,
        "Wizard (#2)",
        "*News (#61)",
        "Welcome to LambdaCore",
        "",
        "Getting Started with your LambdaCore MOO",
        "========================================",
        "",
        "Thank you for choosing LambdaCore!",
        "",
        "Initial Setup Notes",
        "-------------------",
        "",
        "The \"welcome\" screen, seen when a player connects.",
        "  -- this is stored in $login.welcome_message",
        "",
        "Do you want on-line character creation?",
        "  -- this is stored in $login.create_enabled",
        "     for more detailed information, edit $login:player_creation_enabled",
        "",
        "Do you want to limit the number of players on the MOO at once?",
        "  -- look at $login.max_connections",
        "     the `connection_limit' message on $login is the message printed",
        "     when this limit is reached.",
        "",
        "Do you want a different default player class?",
        "  -- set $player_class to a different value",
        "     *do not* change $player",
        "",
        "You should also set the following:",
        "  $network.postmaster",
        "    -- your email address, or the email address of the person who will ",
        "       handle your email",
        "  $network.site",
        "    -- the machine your MOO is running on (e.g. \"lambda.moo.mud.org\")",
        "  $network.port",
        "    -- the port your MOO is running on (e.g. 8888)",
        "  $network.MOO_Name",
        "    -- the name of your MOO (e.g. \"LambdaMOO\")",
        "  $site_db.domain",
        "  -- this is set to the `domain' of your address",
        "     (eg `foo.com' for `moo.foo.com')",
        "",
        "If you compiled the server with open_network_connection() enabled (allowing the MOO to open up connections with other computers on the network), then you should set",
        "  $network.active = 1",
        "     This will enable @newpassword, @registerme, @password, @mailme, @netforward, and others to send mail from the MOO.",
        "",
        "-------------------------------------------------------------------",
        "",
        "Setting Yourself Up",
        "-------------------",
        "",
        "Set a password for yourself.",
        "  -- @password <new-password>",
        "",
        "Set a description for yourself.",
        "  -- @describe me as <anything>",
        "",
        "Set a gender for yourself.",
        "  -- @gender <gender>",
        "",
        "There are, also, a large number of messages you can set on yourself.  Setting them will enhance the virtual reality.",
        "",
        "-------------------------------------------------------------------",
        "",
        "About Guests",
        "------------",
        "",
        "To make a new Guest character:",
        "  -- @make-guest <guestname>",
        "     will make a new guest with the name you specify with `_Guest' appended",
        "     and some other standard but useful aliases",
        "",
        "This is the easiest way to make Guest characters.  The most important things to remember about Guests, if you want to make them yourself, are:",
        "  -- make them owned by nonwizards, and not owned by themselves",
        "  -- make sure they've got .password == 0, and that .password is nonclear",
        "  -- at least one Guest must always be named `Guest'; this can be an alias",
        "",
        "To set the default description and gender for a guest:",
        "  -- set .default_description to the description the guest should start with",
        "  -- set .default_gender to the gender the guest should start with",
        "  -- remember to set .description and .gender too, for the guest's first use",
        "",
        "-------------------------------------------------------------------",
        "",
        "Adding to the Newspaper",
        "-----------------------",
        "",
        "The newspaper is a special mailing list.  To add a post to the newspaper, send mail to it (as *News or $news), and then note the number of your post (let's call it <x> and:",
        "  -- @addnews <x> to *News",
        "... in general, `@addnews $ to *News' will work as well.",
        "",
        "-------------------------------------------------------------------",
        "",
        "Quota",
        "-----",
        "",
        "By default, LambdaCore runs with byte-based quota, an in-DB quota system, limiting users by total database space as opposed to total objects.  You'll need to do two things:",
        "  -- decide on the default quota:",
        "     ;$byte_quota_utils.default_quota[1] = <a number of bytes>",
        "  -- start the measurement task; see `help routine_tasks' for more information (Note: this help topic contains information about more than just the quota task; it should be read regardless of how quota is set).",
        "",
        "If you prefer the quota system documented in the LambdaMOO Programmer's Manual, directly supported by the server, you can enable object-based quota:",
        "  -- set $quota_utils to $object_quota_utils",
        "",
        "It's best that you make this switch before users start, because converting existing users is an awkward (and inherently arbitrary and political) move.",
        "",
        "-------------------------------------------------------------------",
        "",
        "Making Programmers",
        "------------------",
        "",
        "The command to turn someone into a programmer is `@programmer'  Its syntax is `@programmer <user>'.  For example:",
        "  -- @programmer Haakon",
        "The `@programmer' verb will prompt you if the user isn't set up with a description and a gender.",
        "",
        "No code to automatically grant programmer bits is included with LambdaCore.",
        "",
        "Making Wizards",
        "--------------",
        "",
        "THINK CAREFULLY.",
        "",
        "Be very careful before giving someone a wizard bit.  That person can do gross damage to your database, and fixable but serious damage to the machine it runs on.  That person can quite possibly open outbound network connections from your machine, and thus commit acts for which your host system will be blamed.  That person can ruin your MOO's as-yet-untarnished reputation.",
        "",
        "Wizards have technical power, the ability to change anything within the database, to create anything within the database.  Be careful with the idea of a `Social Wizard' -- a nontechnical person holding a wizard bit is fairly likely to, at some point, accidentally do something destructive.  It's a good idea not to socialize as your wizard character, for the same reason, to make it less likely to be accidentally destructive.",
        "",
        "That said, in general you don't turn an existing character into a wizard, you make a -new- character to be the wizard.  This is because the existing character probably owns code and objects which could be destructive if suddenly made wizardly; it's a good security measure to make a fresh player.  So, to make a fresh player:",
        "  -- @make-player (see `help @make-player' for more information)",
        "     this will make you a new player. for this example, #123",
        "",
        "To make #123 a wizard:",
        "  -- @programmer #123",
        "     (a nonprogrammer wizard is a truly strange beast)",
        "  -- ;#123.wizard = 1;",
        "  -- @chparent #123 to $wiz",
        "  -- ;#123.public_identity = <the player's nonwizard character's object number>",
        "",
        "-------------------------------------------------------------------",
        "",
        "Good luck with your new LambdaCore database!",
        "",
        "Visit us at LambdaMOO: lambda.moo.mud.org 8888",
        "",
        "Join the international mailing list for MOO coders: send an email message to moo-cows-request@the-b.org with the word `subscribe' as the body of your message.",
        "",
        "Do good things.",
        "",
        "The LambdaMOO Wizards",
        "[authored February 15, 1999]"
      }
    }
  };
  override moderated = 1;
  override object_size = {21017, 1084848672};
  override readers = 1;

  verb description (this none this) owner: HACKER flags: "rxd"
    raw = ctime(this.last_news_time);
    "         111111111122222";
    "123456789012345678901234";
    "Fri Nov 30 14:31:21 1990";
    date = raw[1..10] + "," + raw[20..24];
    return strsub(this.description, "%d", date);
  endverb

  verb is_writable_by (this none this) owner: #2 flags: "rxd"
    return pass(@args) || args[1] in $list_utils:map_prop($object_utils:descendants($wiz), "mail_identity");
  endverb

  verb rm_message_seq (this none this) owner: HACKER flags: "rxd"
    if (this:ok_write(caller, caller_perms()))
      seq = args[1];
      this.current_news_going = $seq_utils:intersection(this.current_news, seq);
      this.current_news = $seq_utils:contract(this.current_news, seq);
      return $mail_agent:(verb)(@args);
    else
      return E_PERM;
    endif
  endverb

  verb undo_rmm (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    seq = $mail_agent:(verb)(@args);
    this.current_news = $seq_utils:union(this.current_news_going, $seq_utils:expand(this.current_news, seq));
    this.current_news_going = {};
    return seq;
  endverb

  verb expunge_rmm (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    endif
    this.current_news_going = {};
    return $mail_agent:(verb)(@args);
  endverb

  verb set_current_news (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    else
      this.current_news = new = args[1];
      if (new)
        newlast = $seq_utils:last(new);
        newlasttime = this:messages_in_seq(newlast)[2][1];
        if (newlasttime > this.last_news_time)
          "... only notify people if there exists a genuinely new item...";
          this.last_news_time = newlasttime;
          this:touch();
        endif
      else
        "...flush everything...";
        this.last_news_time = 0;
      endif
    endif
  endverb

  verb add_current_news (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    else
      return this:set_current_news($seq_utils:union(this.current_news, args[1]));
    endif
  endverb

  verb rm_current_news (this none this) owner: HACKER flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      return E_PERM;
    else
      return this:set_current_news($seq_utils:intersection(this.current_news, $seq_utils:complement(args[1])));
    endif
  endverb

  verb news_display_seq_full (this none this) owner: #2 flags: "rxd"
    ":news_display_seq_full(msg_seq) => {cur, last-read-date}";
    "Display the given msg_seq as a collection of news items";
    set_task_perms(caller_perms());
    desc = this:description();
    player:notify(typeof(desc) == LIST ? desc[1] | desc);
    player:notify("");
    msgs = this:messages_in_seq(args[1]);
    for i in [-(n = length(msgs))..-1]
      x = msgs[-i];
      player:notify_lines(this:to_text(@x[2]));
      player:notify("");
      $command_utils:suspend_if_needed(0);
    endfor
    player:notify("(end)");
    return {msgs[n][1], msgs[n][2][1]};
  endverb

  verb to_text (this none this) owner: HACKER flags: "rxd"
    ":to_text(@msg) => message in text form -- formatted like a $news entry circa October, 1993";
    date = args[1];
    by = args[2];
    "by = by[1..index(by, \"(\") - 2]";
    subject = args[4] == " " ? "-*-NEWS FLASH-*-" | $string_utils:uppercase(args[4]);
    text = args[("" in {@args, ""}) + 1..$];
    ctime = $time_utils:time_sub("$D, $N $3, $Y", date);
    return {ctime, subject, @text};
    return {subject, tostr("  by ", by, " on ", ctime), "", @text};
  endverb

  verb check (this none this) owner: #2 flags: "rxd"
    set_task_perms(caller_perms());
    if ((player:get_current_message(this) || {0, 0})[2] < this.last_news_time)
      if ((n = player:mail_option("news")) in {0, "all"})
        player:tell("There is new news.  Type `news' to read all news or `news new' to read just new news.");
      elseif (n == "contents")
        player:tell("There is new news.  Type `news all' to read all news or `news new' to read just new news.");
      elseif (n == "new")
        player:tell("There is new news.  Type `news' to read new news, or `news all' to read all news.");
      endif
    endif
  endverb

  verb touch (this none none) owner: #2 flags: "rxd"
    if (!this:ok_write(caller, valid(who = caller_perms()) ? who | player))
      player:notify("Permission denied.");
      return;
    endif
    fork (0)
      for p in (connected_players())
        $command_utils:suspend_if_needed(0);
        if ((p:get_current_message(this) || {0, 0})[2] < this.last_news_time)
          p:notify("There's a new edition of the newspaper.  Type 'news new' to see the new article(s).");
        endif
      endfor
    endfork
  endverb

  verb "@addnews" (any at this) owner: #2 flags: "rxd"
    if (caller_perms() != #-1 && caller_perms() != player)
      raise(E_PERM);
    endif
    set_task_perms(player);
    if (!this:is_writable_by(player))
      player:notify("You can't write the news.");
    elseif (typeof(result = this:add_news(args[1..(prepstr in args) - 1], player:get_current_message(this) || {0, 0})) == STR)
      player:notify(result);
    else
      new = this.current_news;
      if (new)
        player:notify("Current newspaper set.");
        this:display_seq_headers(new);
      else
        player:notify("Current newspaper is now empty.");
      endif
    endif
  endverb

  verb "@rmnews" (any from this) owner: #2 flags: "rxd"
    if (caller_perms() != #-1 && caller_perms() != player)
      raise(E_PERM);
    endif
    set_task_perms(player);
    if (!this:is_writable_by(player))
      player:notify("You can't write the news.");
    elseif (typeof(result = this:rm_news(args[1..(prepstr in args) - 1], player:get_current_message(this) || {0, 0})) == STR)
      player:notify(result);
    else
      new = this.current_news;
      if (new)
        player:notify("Current newspaper set.");
        this:display_seq_headers(new);
      else
        player:notify("Current newspaper is now empty.");
      endif
    endif
  endverb

  verb "@setnews" (this at any) owner: #2 flags: "rd"
    set_task_perms(player);
    if (!this:is_writable_by(player))
      player:notify("You can't write the news.");
    elseif (typeof(seq = this:_parse(strings = args[(prepstr in args) + 1..$], @player:get_current_message(this) || {0, 0})) == STR)
      player:notify(seq);
    else
      old = this.current_news;
      if (old == seq)
        player:notify("No change.");
      else
        this:set_current_news(seq);
        if (seq)
          player:notify("Current newspaper set.");
          this:display_seq_headers(seq);
        else
          player:notify("Current newspaper is now empty.");
        endif
      endif
    endif
  endverb

  verb _parse (this none this) owner: HACKER flags: "rxd"
    if (!(strings = args[1]))
      return "You need to specify a message sequence";
    elseif (typeof(pms = this:parse_message_seq(@args)) == STR)
      return $string_utils:substitute(pms, {{"%f", "The news"}, {"%<has>", "has"}, {"%%", "%"}});
    elseif (typeof(pms) != LIST)
      return tostr(pms);
    elseif (length(pms) > 1)
      return tostr("I don't understand `", pms[2], "'.");
    elseif (!(seq = pms[1]))
      return tostr("The News (", this, ") has no `", $string_utils:from_list(strings, " "), "' messages.");
    else
      return seq;
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.description = "It's the current issue of the News, dated %d.";
      this.moderated = 1;
      this.last_news_time = 0;
      this.readers = 1;
      this.expire_period = 0;
      this.archive_news = {};
      $mail_agent:send_message(#2, this, "Welcome to LambdaCore", $wiz_utils.new_core_message);
      this:add_news("$");
    else
      return E_PERM;
    endif
  endverb

  verb add_news (this none this) owner: #2 flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      $error:raise(E_PERM);
    endif
    {specs, ?cur = {0, 0}} = args;
    seq = this:_parse(specs, @cur);
    if (typeof(seq) == STR)
      return seq;
    endif
    old = this.current_news;
    new = $seq_utils:union(old, seq);
    if (old == new)
      return "Those messages are already in the news.";
    endif
    this:set_current_news(new);
    return 1;
  endverb

  verb rm_news (this none this) owner: #2 flags: "rxd"
    if (!this:ok_write(caller, caller_perms()))
      raise(E_PERM);
    endif
    {specs, ?cur = {0, 0}} = args;
    seq = this:_parse(specs, @cur);
    if (typeof(seq) == STR)
      return seq;
    endif
    old = this.current_news;
    new = $seq_utils:intersection(old, $seq_utils:complement(seq));
    if (old == new)
      return "Those messages were not in the news.";
    endif
    this:set_current_news(new);
    return 1;
  endverb

  verb "@listnews" (none on this) owner: #2 flags: "rxd"
    player:notify("The following articles are currently in the newspaper:");
    this:display_seq_headers(this.current_news);
  endverb

  verb "@clearnews" (this none none) owner: #2 flags: "rd"
    set_task_perms(player);
    if (this:is_writable_by(player))
      this:set_current_news({});
      player:notify("Current newspaper is now empty.");
    else
      player:notify("You can't write the news.");
    endif
  endverb
endobject