object LOGIN
  name: "Login Commands"
  parent: ROOT_CLASS
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  property blacklist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property blank_command (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = "welcome";
  property bogus_command (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = "?";
  property boot_process (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property checkpoint_in_progress (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property connection_limit_msg (owner: HACKER, flags: "r") = "*** The MOO is too busy! The current lag is %l; there are %n connected.  WAIT FIVE MINUTES BEFORE TRYING AGAIN.";
  property create_enabled (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 1;
  property current_connections (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {AMBIGUOUS_MATCH};
  property current_lag (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property current_numcommands (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {1};
  property downtimes (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {{1529543472, 0}, {1529444307, 0}};
  property goaway_message (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {
    "                          ***************************",
    "                          *  Welcome to LagdaMOO!  *",
    "                          ***************************",
    "                                       ",
    "                      Running Version %v of LagdaMOO",
    "",
    "PLEASE NOTE:",
    "   LagdaMOO is a new kind of society, where thousands of people voluntarily",
    "come together from all over the world.  What these people say or do may not",
    "always be to your liking; as when visiting any international city, it is wise",
    "to be careful who you associate with and what you say.",
    "   The operators of LagdaMOO have provided the materials for the buildings of",
    "this community, but are not responsible for what is said or done in them.  In",
    "particular, you must assume responsibility if you permit minors or others to",
    "access LagdaMOO through your facilities.  The statements and viewpoints",
    "expressed here are not necessarily those of the wizards, Pavel Curtis, ",
    "Stanford University, or Placeware Inc., and those parties disclaim any ",
    "responsibility for them.",
    "",
    "NOTICE FOR JOURNALISTS AND RESEARCHERS:",
    "  The citizens of LagdaMOO request that you ask for permission from all",
    "direct participants before quoting any material collected here.",
    "",
    "For assistance either now or later, type `help'."
  };
  property graylist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property help_message (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "# Getting Started

To sign in to an existing account, use your **player name** and **password**.

To create a new account, choose a unique player name and password.

## Available commands (for telnet users)

- `connect <name> <password>` - Sign in to an existing account
- `create <name> <password>` - Create a new account
- `who` - See who is currently connected
- `quit` - Disconnect from the server

For more detailed help once you're logged in, type `help` after connecting.";
  property help_message_content_type (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "text/djot";
  property ignored (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property intercepted_actions (owner: HACKER, flags: "") = {};
  property intercepted_players (owner: HACKER, flags: "") = {};
  property lag_cutoff (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 5;
  property lag_exemptions (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {};
  property lag_sample_interval (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 15;
  property lag_samples (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = {0, 0, 0, 0, 0};
  property last_lag_sample (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property max_connections (owner: HACKER, flags: "rc") = 99999;
  property max_numcommands (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 20;
  property max_player_name (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 40;
  property newt_registration_string (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "Your character is temporarily hosed.";
  property newted (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {};
  property print_lag (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property redlist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property registration_address (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "";
  property registration_string (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "Character creation is disabled.";
  property request_enabled (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = 0;
  property spooflist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property temporary_blacklist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property temporary_graylist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property temporary_newts (owner: BYTE_QUOTA_UTILS_WORKING, flags: "c") = {};
  property temporary_redlist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property temporary_spooflist (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = {{}, {}};
  property welcome_message (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "# Welcome to the LambdaCore database

To get started, either **sign in** to an existing account or **create a new one**.

For more information, tap the help button or type `help`.

---

_Administrators: You may want to customize this text and the help message, which are stored in `$login.welcome_message` and `$login.help_message`._";
  property welcome_message_content_type (owner: BYTE_QUOTA_UTILS_WORKING, flags: "rc") = "text/djot";
  property who_masks_wizards (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = 0;

  override aliases = {"Login Commands"};
  override description = "This provides everything needed by #0:do_login_command.  See `help $login' on $core_help for details.";
  override object_size = {42064, 1084848672};

  verb "?" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      clist = {};
      for j in ({this, @$object_utils:ancestors(this)})
        for i in [1..length(verbs(j))]
          if (verb_args(j, i) == {"any", "none", "any"} && index((info = verb_info(j, i))[2], "x"))
            vname = $string_utils:explode(info[3])[1];
            star = index(vname + "*", "*");
            clist = {@clist, $string_utils:uppercase(vname[1..star - 1]) + strsub(vname[star..$], "*", "")};
          endif
        endfor
      endfor
      notify(player, "I don't understand that.  Valid commands at this point are");
      notify(player, "   " + $string_utils:english_list(setremove(clist, "?"), "", " or "));
      return 0;
    endif
  endverb

  verb "wel*come @wel*come" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      msg = this.welcome_message;
      content_type = $object_utils:has_property(this, "welcome_message_content_type") ? this.welcome_message_content_type | "text/plain";
      version = server_version();
      if (typeof(msg) == STR)
        notify(player, strsub(msg, "%v", version), 0, 0, content_type);
      else
        for line in (msg)
          if (typeof(line) == STR)
            notify(player, strsub(line, "%v", version));
          endif
        endfor
      endif
      this:check_player_db();
      this:check_for_shutdown();
      this:check_for_checkpoint();
      this:maybe_print_lag();
      return 0;
    endif
  endverb

  verb "w*ho @w*ho" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    masked = $login.who_masks_wizards ? $wiz_utils:connected_wizards() | {};
    if (caller != #0 && caller != this)
      return E_PERM;
    elseif (!args)
      plyrs = connected_players();
      if (length(plyrs) > 100)
        this:notify(tostr("You have requested a listing of ", length(plyrs), " players.  Please restrict the number of players in any single request to a smaller number.  The lag thanks you."));
        return 0;
      else
        $code_utils:show_who_listing($set_utils:difference(plyrs, masked)) || this:notify("No one logged in.");
      endif
    else
      plyrs = listdelete($command_utils:player_match_result($string_utils:match_player(args), args), 1);
      if (length(plyrs) > 100)
        this:notify(tostr("You have requested a listing of ", length(plyrs), " players.  Please restrict the number of players in any single request to a smaller number.  The lag thanks you."));
        return 0;
      endif
      $code_utils:show_who_listing(plyrs, $set_utils:intersection(plyrs, masked));
    endif
    return 0;
  endverb

  verb "co*nnect @co*nnect" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$login:connect(player-name [, password])";
    " => 0 (for failed connections)";
    " => objnum (for successful connections)";
    caller == #0 || caller == this || raise(E_PERM);
    "=================================================================";
    "=== Check arguments, print usage notice if necessary";
    try
      {name, ?password = 0} = args;
      name = strsub(name, " ", "_");
    except (E_ARGS)
      notify(player, tostr("Usage:  ", verb, " <existing-player-name> <password>"));
      return 0;
    endtry
    try
      "=================================================================";
      "=== Is our candidate name invalid?";
      if (!valid(candidate = orig_candidate = this:_match_player(name)))
        raise(E_INVARG, tostr("`", name, "' matches no player name."));
      endif
      "=================================================================";
      "=== Is our candidate unable to connect for generic security";
      "=== reasons (ie clear password, non-player object)?";
      if (`is_clear_property(candidate, "password") ! E_PROPNF' || !$object_utils:isa(candidate, $player))
        server_log(tostr("FAILED CONNECT: ", name, " (", candidate, ") on ", connection_name(player), $string_utils:connection_hostname(connection_name(player)) in candidate.all_connect_places ? "" | "******"));
        raise(E_INVARG);
      endif
      "=================================================================";
      "=== Check password";
      if (typeof(cp = candidate.password) == STR)
        "=== Candidate requires a password";
        if (password)
          "=== Candidate requires a password, and one was provided";
          if (!argon2_verify(cp, password))
            "=== Candidate requires a password, and one was provided, which was wrong";
            server_log(tostr("FAILED CONNECT: ", name, " (", candidate, ") on ", connection_name(player), $string_utils:connection_hostname(connection_name(player)) in candidate.all_connect_places ? "" | "******"));
            raise(E_INVARG, "Invalid password.");
          else
            "=== Candidate requires a password, and one was provided, which was right";
          endif
        else
          "=== Candidate requires a password, and none was provided";
          set_connection_option(player, "binary", 1);
          notify(player, "Password: ");
          set_connection_option(player, "binary", 0);
          set_connection_option(player, "client-echo", 0);
          this:add_interception(player, "intercepted_password", candidate);
          return 0;
        endif
      elseif (cp == 0)
        "=== Candidate does not require a password";
      else
        "=== Candidate has a nonstandard password; something's wrong";
        raise(E_INVARG);
      endif
      "=================================================================";
      "=== Is the player locked out?";
      if ($no_connect_message && !candidate.wizard)
        notify(player, $no_connect_message);
        server_log(tostr("REJECTED CONNECT: ", name, " (", candidate, ") on ", connection_name(player)));
        return 0;
      endif
      "=================================================================";
      "=== Check guest connections";
      if ($object_utils:isa(candidate, $guest) && !valid(candidate = candidate:defer()))
        if (candidate == #-2)
          server_log(tostr("GUEST DENIED: ", connection_name(player)));
          notify(player, "Sorry, guest characters are not allowed from your site at the current time.");
        else
          notify(player, "Sorry, all of our guest characters are in use right now.");
        endif
        return 0;
      endif
      "=================================================================";
      "=== Check newts";
      if (candidate in this.newted)
        if (entry = $list_utils:assoc(candidate, this.temporary_newts))
          if ((uptime = this:uptime_since(entry[2])) > entry[3])
            "Temporary newting period is over.  Remove entry.  Oh, send mail, too.";
            this.temporary_newts = setremove(this.temporary_newts, entry);
            this.newted = setremove(this.newted, candidate);
            fork (0)
              player = this.owner;
              $mail_agent:send_message(player, $newt_log, tostr("automatic @unnewt ", candidate.name, " (", candidate, ")"), {"message sent from $login:connect"});
            endfork
          else
            notify(player, "");
            notify(player, this:temp_newt_registration_string(entry[3] - uptime));
            boot_player(player);
            return 0;
          endif
        else
          notify(player, "");
          notify(player, this:newt_registration_string());
          boot_player(player);
          return 0;
        endif
      endif
      "=================================================================";
      "=== Connection limits based on lag";
      if (!candidate.wizard && !(candidate in this.lag_exemptions) && (howmany = length(connected_players())) >= (max = this:max_connections()) && !$object_utils:connected(candidate))
        notify(player, $string_utils:subst(this.connection_limit_msg, {{"%n", tostr(howmany)}, {"%m", tostr(max)}, {"%l", tostr(this:current_lag())}, {"%t", candidate.last_connect_attempt ? ctime(candidate.last_connect_attempt) | "not recorded"}}));
        if ($object_utils:has_property($local, "mudlist"))
          notify(player, "You may wish to try another MUD while waiting for the MOO to unlag.  Here are a few that we know of:");
          for l in ($local.mudlist:choose(3))
            notify(player, l);
          endfor
        endif
        candidate.last_connect_attempt = time();
        server_log(tostr("CONNECTION LIMIT EXCEEDED: ", name, " (", candidate, ") on ", connection_name(player)));
        boot_player(player);
        return 0;
      endif
      "=================================================================";
      "=== Log the player on!";
      if (candidate != orig_candidate)
        notify(player, tostr("Okay,... ", name, " is in use.  Logging you in as `", candidate.name, "'"));
      endif
      this:record_connection(candidate);
      return candidate;
    except (E_INVARG)
      notify(player, "Either that player does not exist, or has a different password.");
      return 0;
    endtry
  endverb

  verb "cr*eate @cr*eate" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
      "... caller isn't :do_login_command()...";
    elseif (!this:player_creation_enabled(player))
      notify(player, this:registration_string());
      "... we've disabled player creation ...";
    elseif (length(args) != 2)
      notify(player, tostr("Usage:  ", verb, " <new-player-name> <new-password>"));
    elseif ($player_db.frozen)
      notify(player, "Sorry, can't create any new players right now.  Try again in a few minutes.");
    elseif (!(name = args[1]) || name == "<>")
      notify(player, "You can't have a blank name!");
      if (name)
        notify(player, "Also, don't use angle brackets (<>).");
      endif
    elseif (name[1] == "<" && name[$] == ">")
      notify(player, "Try that again but without the angle brackets, e.g.,");
      notify(player, tostr(" ", verb, " ", name[2..$ - 1], " ", strsub(strsub(args[2], "<", ""), ">", "")));
      notify(player, "This goes for other commands as well.");
    elseif (index(name, " "))
      notify(player, "Sorry, no spaces are allowed in player names.  Use dashes or underscores.");
      "... lots of routines depend on there not being spaces in player names...";
    elseif (!$player_db:available(name) || this:_match_player(name) != $failed_match)
      notify(player, "Sorry, that name is not available.  Please choose another.");
      "... note the :_match_player call is not strictly necessary...";
      "... it is merely there to handle the case that $player_db gets corrupted.";
    elseif (!(password = args[2]))
      notify(player, "You must set a password for your player.");
    else
      new = $quota_utils:bi_create($player_class, $nothing);
      set_player_flag(new, 1);
      new.name = name;
      new.aliases = {name};
      new.programmer = $player_class.programmer;
      salt_str = salt();
      new.password = argon2(password, salt_str);
      new.last_password_time = time();
      new.last_connect_time = $maxint;
      "Last disconnect time is creation time, until they login.";
      new.last_disconnect_time = time();
      "make sure the owership quota isn't clear!";
      $quota_utils:initialize_quota(new);
      this:record_connection(new);
      $player_db:insert(name, new);
      `move(new, $player_start) ! ANY';
      return new;
    endif
    return 0;
  endverb

  verb "q*uit @q*uit" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      boot_player(player);
      return 0;
    endif
  endverb

  verb oauth2_check (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$login:oauth2_check(provider, external_id)";
    " => 0 (for not found)";
    " => objnum (for existing OAuth2 identity)";
    caller == #0 || caller == this || raise(E_PERM);
    try
      {provider, external_id} = args;
    except (E_ARGS)
      notify(player, "OAuth2 check failed: invalid arguments");
      return 0;
    endtry
    if (valid(candidate = this:find_by_oauth2(provider, external_id)))
      server_log(tostr("OAUTH2 CHECK SUCCESS: ", provider, ":", external_id, " -> ", candidate));
      this:record_connection(candidate);
      return candidate;
    else
      server_log(tostr("OAUTH2 CHECK NOT FOUND: ", provider, ":", external_id));
      return 0;
    endif
  endverb

  verb oauth2_create (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$login:oauth2_create(provider, external_id, email, name, username, player_name)";
    " => 0 (for failed creation)";
    " => objnum (for successful creation)";
    caller == #0 || caller == this || raise(E_PERM);
    if (!this:player_creation_enabled(player))
      notify(player, this:registration_string());
      return 0;
    endif
    try
      {provider, external_id, email, name, username, player_name} = args;
    except (E_ARGS)
      notify(player, "OAuth2 create failed: invalid arguments");
      return 0;
    endtry
    if ($player_db.frozen)
      notify(player, "Sorry, can't create any new players right now.  Try again in a few minutes.");
      return 0;
    elseif (!player_name || player_name == "<>")
      notify(player, "You can't have a blank name!");
      return 0;
    elseif (player_name[1] == "<" && player_name[$] == ">")
      notify(player, "Don't use angle brackets in your player name.");
      return 0;
    elseif (index(player_name, " "))
      notify(player, "Sorry, no spaces are allowed in player names.  Use dashes or underscores.");
      return 0;
    elseif (!$player_db:available(player_name) || this:_match_player(player_name) != $failed_match)
      notify(player, "Sorry, that name is not available.  Please choose another.");
      return 0;
    endif
    new = $quota_utils:bi_create($player_class, $nothing);
    set_player_flag(new, 1);
    new.name = player_name;
    new.aliases = {player_name};
    new.programmer = $player_class.programmer;
    new.password = 0;
    new.email_address = email;
    new.oauth2_identities = {{provider, external_id}};
    new.last_connect_time = $maxint;
    new.last_disconnect_time = time();
    $quota_utils:initialize_quota(new);
    this:record_connection(new);
    $player_db:insert(player_name, new);
    `move(new, $player_start) ! ANY';
    server_log(tostr("OAUTH2 CREATE: ", player_name, " (", new, ") via ", provider, ":", external_id));
    return new;
  endverb

  verb oauth2_connect (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "$login:oauth2_connect(provider, external_id, email, name, username, existing_name, existing_password)";
    " => 0 (for failed connection)";
    " => objnum (for successful link)";
    caller == #0 || caller == this || raise(E_PERM);
    try
      {provider, external_id, email, name, username, existing_name, existing_password} = args;
      server_log(tostr("OAUTH2 CONNECT ATTEMPT: provider=", provider, " external_id=", external_id, " existing_name=", existing_name, " args_count=", length(args)));
    except (E_ARGS)
      server_log(tostr("OAUTH2 CONNECT E_ARGS: received ", length(args), " args, expected 7"));
      notify(player, "OAuth2 connect failed: invalid arguments");
      return 0;
    endtry
    if (!valid(candidate = this:_match_player(existing_name)))
      server_log(tostr("OAUTH2 CONNECT FAILED: player not found: ", existing_name));
      notify(player, "That player does not exist.");
      return 0;
    endif
    server_log(tostr("OAUTH2 CONNECT: found candidate ", candidate, " password_type=", typeof(candidate.password)));
    if (typeof(cp = candidate.password) == STR)
      "=== Candidate has a password, verify it";
      if (!argon2_verify(cp, existing_password))
        server_log(tostr("OAUTH2 CONNECT FAILED PASSWORD: ", existing_name, " (", candidate, ")"));
        notify(player, "Invalid password for existing account.");
        return 0;
      endif
      server_log(tostr("OAUTH2 CONNECT: password verified for ", existing_name));
    elseif (cp == 0)
      "=== Candidate has no password set, allow linking";
      server_log(tostr("OAUTH2 CONNECT: no password required for ", existing_name, " (", candidate, ")"));
    else
      "=== Candidate has nonstandard password";
      server_log(tostr("OAUTH2 CONNECT FAILED: nonstandard password type for ", existing_name, " (", candidate, ")"));
      notify(player, "Cannot link to that account.");
      return 0;
    endif
    if ($object_utils:has_property(candidate, "oauth2_identities"))
      for identity in (candidate.oauth2_identities)
        if (typeof(identity) == LIST && length(identity) == 2)
          if (identity[1] == provider && identity[2] == external_id)
            notify(player, "This OAuth2 identity is already linked to this account.");
            this:record_connection(candidate);
            return candidate;
          endif
        endif
      endfor
      candidate.oauth2_identities = {@candidate.oauth2_identities, {provider, external_id}};
    else
      candidate.oauth2_identities = {{provider, external_id}};
    endif
    "=== Set email address if one was provided and the candidate doesn't have one";
    if (email && (!$object_utils:has_property(candidate, "email_address") || !candidate.email_address))
      candidate.email_address = email;
      server_log(tostr("OAUTH2 CONNECT: set email address for ", existing_name, " to ", email));
    endif
    this:record_connection(candidate);
    server_log(tostr("OAUTH2 CONNECT: ", existing_name, " (", candidate, ") linked ", provider, ":", external_id));
    return candidate;
  endverb

  verb "up*time @up*time" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      notify(player, tostr("The server has been up for ", $time_utils:english_time(time() - $last_restart_time), "."));
      return 0;
    endif
  endverb

  verb "v*ersion @v*ersion" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      notify(player, tostr("The MOO is currently running version ", server_version(), " of the LambdaMOO server code."));
      return 0;
    endif
  endverb

  verb parse_command (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":parse_command(@args) => {verb, args}";
    "Given the args from #0:do_login_command,";
    "  returns the actual $login verb to call and the args to use.";
    "Commands available to not-logged-in users should be located on this object and given the verb_args \"any none any\"";
    if (caller != #0 && caller != this)
      return E_PERM;
    endif
    if (li = this:interception(player))
      return {@li, @args};
    endif
    if (!args)
      return {this.blank_command, @args};
    elseif ((verb = args[1]) && !$string_utils:is_numeric(verb))
      for i in ({this, @$object_utils:ancestors(this)})
        try
          if (verb_args(i, verb) == {"any", "none", "any"} && index(verb_info(i, verb)[2], "x"))
            return args;
          endif
        except (ANY)
          continue i;
        endtry
      endfor
    endif
    return {this.bogus_command, @args};
  endverb

  verb check_for_shutdown (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    when = $shutdown_time - time();
    if (when >= 0)
      line = "***************************************************************************";
      notify(player, "");
      notify(player, "");
      notify(player, line);
      notify(player, line);
      notify(player, "****");
      notify(player, "****  WARNING:  The server will shut down in " + $time_utils:english_time(when - when % 60) + ".");
      for piece in ($generic_editor:fill_string($shutdown_message, 60))
        notify(player, "****            " + piece);
      endfor
      notify(player, "****");
      notify(player, line);
      notify(player, line);
      notify(player, "");
      notify(player, "");
    endif
  endverb

  verb check_player_db (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if ($player_db.frozen)
      line = "***************************************************************************";
      notify(player, "");
      notify(player, line);
      notify(player, "***");
      for piece in ($generic_editor:fill_string("The character-name matcher is currently being reloaded.  This means your character name might not be recognized even though it still exists.  Don't panic.  You can either wait for the reload to finish or you can connect using your object number if you remember it (e.g., `connect #1234 yourpassword').", 65))
        notify(player, "***       " + piece);
      endfor
      notify(player, "***");
      for piece in ($generic_editor:fill_string("Repeat:  Do not panic.  In particular, please do not send mail to any wizards or the registrar asking about this.  It will finish in time.  Thank you for your patience.", 65))
        notify(player, "***       " + piece);
      endfor
      if (this:player_creation_enabled(player))
        notify(player, "***       This also means that character creation is disabled.");
      endif
      notify(player, "***");
      notify(player, line);
      notify(player, "");
    endif
  endverb

  verb _match_player (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":_match_player(name)";
    "This is the matching routine used by @connect.";
    "returns either a valid player corresponding to name or $failed_match.";
    name = args[1];
    if (valid(candidate = $string_utils:literal_object(name)) && is_player(candidate))
      return candidate;
    endif
    ".....uncomment this to trust $player_db and have `connect' recognize aliases";
    if (valid(candidate = $player_db:find_exact(name)) && is_player(candidate))
      return candidate;
    endif
    ".....uncomment this if $player_db gets hosed and you want by-name login";
    ". for candidate in (players())";
    ".   if (candidate.name == name)";
    ".     return candidate; ";
    ".   endif ";
    ". endfor ";
    ".....";
    return $failed_match;
  endverb

  verb find_by_oauth2 (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":find_by_oauth2(provider, external_id)";
    "Search all players for matching oauth2_identities entry";
    "Returns player object or $failed_match";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {provider, external_id} = args;
    for candidate in (players())
      if ($object_utils:has_property(candidate, "oauth2_identities"))
        for identity in (candidate.oauth2_identities)
          if (typeof(identity) == LIST && length(identity) == 2)
            if (identity[1] == provider && identity[2] == external_id)
              return candidate;
            endif
          endif
        endfor
      endif
    endfor
    return $failed_match;
  endverb

  verb notify (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    set_task_perms(caller_perms());
    `notify(player, args[1]) ! ANY';
  endverb

  verb tell (this none this) owner: HACKER flags: "rxd"
    "keeps bad things from happening if someone brings this object into a room and talks to it.";
    return 0;
  endverb

  verb player_creation_enabled (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Accepts a player object.  If player creation is enabled for that player object, then return true.  Otherwise, return false.";
    "Default implementation checks the player's connecting host via $login:blacklisted to decide.";
    if (caller_perms().wizard)
      return this.create_enabled && !this:blacklisted($string_utils:connection_hostname(connection_name(args[1])));
    else
      return E_PERM;
    endif
  endverb

  verb "newt_registration_string registration_string" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return $string_utils:subst(this.(verb), {{"%e", this.registration_address}, {"%%", "%"}});
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      this.current_lag = 0;
      this.lag_exemptions = {};
      this.max_connections = 99999;
      this.lag_samples = {0, 0, 0, 0, 0};
      this.print_lag = 0;
      this.last_lag_sample = 0;
      this.bogus_command = "?";
      this.blank_command = "welcome";
      this.create_enabled = 1;
      this.registration_address = "";
      this.registration_string = "Character creation is disabled.";
      this.newt_registration_string = "Your character is temporarily hosed.";
      this.welcome_message = "# Welcome to the LambdaCore database\n\nTo get started, either **sign in** to an existing account or **create a new one**.\n\nFor more information, tap the help button or type `help`.\n\n---\n\n_Administrators: You may want to customize this text and the help message, which are stored in `$login.welcome_message` and `$login.help_message`._";
      this.welcome_message_content_type = "text/djot";
      this.help_message = "# Getting Started\n\nTo sign in to an existing account, use your **player name** and **password**.\n\nTo create a new account, choose a unique player name and password.\n\n## Available commands (for telnet users)\n\n- `connect <name> <password>` - Sign in to an existing account\n- `create <name> <password>` - Create a new account\n- `who` - See who is currently connected\n- `quit` - Disconnect from the server\n\nFor more detailed help once you're logged in, type `help` after connecting.";
      this.help_message_content_type = "text/djot";
      this.redlist = this.blacklist = this.graylist = this.spooflist = {{}, {}};
      this.temporary_redlist = this.temporary_blacklist = this.temporary_graylist = this.temporary_spooflist = {{}, {}};
      this.who_masks_wizards = 0;
      this.newted = this.temporary_newts = {};
      this.downtimes = {};
      if ("monitor" in properties(this))
        delete_property(this, "monitor");
      endif
      if ("monitor" in verbs(this))
        delete_verb(this, "monitor");
      endif
      if ("special_action" in verbs(this))
        set_verb_code(this, "special_action", {});
      endif
      pass(@args);
    endif
  endverb

  verb special_action (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "x"
  endverb

  verb "blacklisted graylisted redlisted spooflisted" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":blacklisted(hostname) => is hostname on the .blacklist";
    ":graylisted(hostname)  => is hostname on the .graylist";
    ":redlisted(hostname)   => is hostname on the .redlist";
    sitelist = this.((this:listname(verb)));
    if (!caller_perms().wizard)
      return E_PERM;
    elseif ((hostname = args[1]) in sitelist[1] || hostname in sitelist[2])
      return 1;
    elseif ($site_db:domain_literal(hostname))
      for lit in (sitelist[1])
        if (index(hostname, lit) == 1 && (hostname + ".")[length(lit) + 1] == ".")
          return 1;
        endif
      endfor
    else
      for dom in (sitelist[2])
        if (index(dom, "*"))
          "...we have a wildcard; let :match_string deal with it...";
          if ($string_utils:match_string(hostname, dom))
            return 1;
          endif
        else
          "...tail of hostname ...";
          if ((r = rindex(hostname, dom)) && (("." + hostname)[r] == "." && r - 1 + length(dom) == length(hostname)))
            return 1;
          endif
        endif
      endfor
    endif
    return this:((verb + "_temp"))(hostname);
  endverb

  verb "blacklist_add*_temp graylist_add*_temp redlist_add*_temp spooflist_add*_temp" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "To add a temporary entry, only call the `temp' version.";
    "blacklist_add_temp(Site, start time, duration)";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {where, ?start, ?duration} = args;
    lname = this:listname(verb);
    which = 1 + !$site_db:domain_literal(where);
    if (index(verb, "temp"))
      lname = "temporary_" + lname;
      this.(lname)[which] = setadd(this.(lname)[which], {where, start, duration});
    else
      this.(lname)[which] = setadd(this.(lname)[which], where);
    endif
    return 1;
  endverb

  verb "blacklist_remove*_temp graylist_remove*_temp redlist_remove*_temp spooflist_remove*_temp" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "The temp version removes from the temporary property if it exists.";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    where = args[1];
    lname = this:listname(verb);
    which = 1 + !$site_db:domain_literal(where);
    if (index(verb, "temp"))
      lname = "temporary_" + lname;
      if (entry = $list_utils:assoc(where, this.(lname)[which]))
        this.(lname)[which] = setremove(this.(lname)[which], entry);
        return 1;
      else
        return E_INVARG;
      endif
    elseif (where in this.(lname)[which])
      this.(lname)[which] = setremove(this.(lname)[which], where);
      return 1;
    else
      return E_INVARG;
    endif
  endverb

  verb listname (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return {"???", "blacklist", "graylist", "redlist", "spooflist"}[1 + index("bgrs", (args[1] || "?")[1])];
  endverb

  verb "who(vanilla)" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0)
      return E_PERM;
    elseif (!args)
      $code_utils:show_who_listing(connected_players()) || this:notify("No one logged in.");
    else
      plyrs = listdelete($command_utils:player_match_result($string_utils:match_player(args), args), 1);
      $code_utils:show_who_listing(plyrs);
    endif
    return 0;
  endverb

  verb record_connection (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":record_connection(plyr) update plyr's connection information";
    "to reflect impending login.";
    if (!caller_perms().wizard)
      return E_PERM;
    else
      plyr = args[1];
      plyr.first_connect_time = min(time(), plyr.first_connect_time);
      plyr.previous_connection = {plyr.last_connect_time, $string_utils:connection_hostname(plyr.last_connect_place)};
      plyr.last_connect_time = time();
      plyr.last_connect_place = cn = connection_name(player);
      chost = $string_utils:connection_hostname(cn);
      acp = setremove(plyr.all_connect_places, chost);
      plyr.all_connect_places = {chost, @acp[1..min($, 15)]};
      if (!$object_utils:isa(plyr, $guest))
        $site_db:add(plyr, chost);
      endif
    endif
  endverb

  verb sample_lag (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    lag = time() - this.last_lag_sample - 15;
    this.lag_samples = {lag, @(this.lag_samples)[1..3]};
    "Now compute the current lag and store it in a property, instead of computing it in :current_lag, which is called a hundred times a second.";
    thislag = max(0, time() - this.last_lag_sample - this.lag_sample_interval);
    if (thislag > 60 * 60)
      "more than an hour, probably the lag sampler stopped";
      this.current_lag = 0;
    else
      samples = this.lag_samples;
      sum = 0;
      cnt = 0;
      for x in (listdelete(samples, 1))
        sum = sum + x;
        cnt = cnt + 1;
      endfor
      this.current_lag = max(thislag, samples[1], samples[2], sum / cnt);
    endif
    fork (15)
      this:sample_lag();
    endfork
    this.last_lag_sample = time();
  endverb

  verb is_lagging (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this:current_lag() > this.lag_cutoff;
  endverb

  verb max_connections (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    max = this.max_connections;
    if (typeof(max) == LIST)
      if (this:is_lagging())
        max = max[1];
      else
        max = max[2];
      endif
    endif
    return max;
  endverb

  verb request_character (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "request_character(player, name, address)";
    "return true if succeeded";
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    {who, name, address} = args;
    connection = $string_utils:connection_hostname(connection_name(who));
    if (reason = $wiz_utils:check_player_request(name, address, connection))
      prefix = "";
      if (reason[1] == "-")
        reason = reason[2..$];
        prefix = "Please";
      else
        prefix = "Please try again, or, to register another way,";
      endif
      notify(who, reason);
      msg = tostr(prefix, " send mail to ", $login.registration_address, ", with the character name you want.");
      for l in ($generic_editor:fill_string(msg, 70))
        notify(who, l);
      endfor
      return 0;
    endif
    if (lines = $no_one:eval_d("$local.help.(\"multiple-characters\")")[2])
      notify(who, "Remember, in general, only one character per person is allowed.");
      notify(who, tostr("Do you already have a ", $network.moo_name, " character? [enter `yes' or `no']"));
      answer = read(who);
      if (answer == "yes")
        notify(who, "Process terminated *without* creating a character.");
        return 0;
      elseif (answer != "no")
        return notify(who, tostr("Please try again; when you get this question, answer `yes' or `no'. You answered `", answer, "'"));
      endif
      notify(who, "For future reference, do you want to see the full policy (from `help multiple-characters'?");
      notify(who, "[enter `yes' or `no']");
      if (read(who) == "yes")
        for x in (lines)
          for y in ($generic_editor:fill_string(x, 70))
            notify(who, y);
          endfor
        endfor
      endif
    endif
    notify(who, tostr("A character named `", name, "' will be created."));
    notify(who, tostr("A random password will be generated, and e-mailed along with"));
    notify(who, tostr(" an explanatory message to: ", address));
    notify(who, tostr(" [Please double-check your email address and answer `no' if it is incorrect.]"));
    notify(who, "Is this OK? [enter `yes' or `no']");
    if (read(who) != "yes")
      notify(who, "Process terminated *without* creating a character.");
      return 0;
    endif
    if (!$network.active)
      $mail_agent:send_message(this.owner, $registration_db.registrar, "Player request", {"Player request from " + connection, ":", "", "@make-player " + name + " " + address});
      notify(who, tostr("Request for new character ", name, " email address '", address, "' accepted."));
      notify(who, tostr("Please be patient until the registrar gets around to it."));
      notify(who, tostr("If you don't get email within a week, please send regular"));
      notify(who, tostr("  email to: ", $login.registration_address, "."));
    elseif ($player_db.frozen)
      notify(who, "Sorry, can't create any new players right now.  Try again in a few minutes.");
    else
      new = $wiz_utils:make_player(name, address);
      password = new[2];
      new = new[1];
      notify(who, tostr("Character ", name, " (", new, ") created."));
      notify(who, tostr("Mailing password to ", address, "; you should get the mail very soon."));
      notify(who, tostr("If you do not get it, please do NOT request another character."));
      notify(who, tostr("Instead, send regular email to ", $login.registration_address, ","));
      notify(who, tostr("with the name of the character you requested."));
      $mail_agent:send_message(this.owner, $new_player_log, tostr(name, " (", new, ")"), {address, tostr(" Automatically created at request of ", valid(player) ? player.name | "unconnected player", " from ", connection, ".")});
      $wiz_utils:send_new_player_mail(tostr("Someone connected from ", connection, " at ", ctime(), " requested a character on ", $network.moo_name, " for email address ", address, "."), name, address, new, password);
      return 1;
    endif
  endverb

  verb "req*uest @req*uest" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    endif
    "must be #0:do_login_command";
    if (!this.request_enabled)
      for line in ($generic_editor:fill_string(this:registration_string(), 70))
        notify(player, line);
      endfor
    elseif (length(args) != 3 || args[2] != "for")
      notify(player, tostr("Usage:  ", verb, " <new-player-name> for <email-address>"));
    elseif ($login:request_character(player, args[1], args[3]))
      boot_player(player);
    endif
  endverb

  verb "h*elp @h*elp" (any none any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != #0 && caller != this)
      return E_PERM;
    else
      msg = this.help_message;
      content_type = $object_utils:has_property(this, "help_message_content_type") ? this.help_message_content_type | "text/plain";
      if (typeof(msg) == STR)
        notify(player, msg, 0, 0, content_type);
      else
        for line in (msg)
          if (typeof(line) == STR)
            notify(player, line);
          endif
        endfor
      endif
      return 0;
    endif
  endverb

  verb maybe_print_lag (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller == this || caller_perms() == player)
      if (this.print_lag)
        lag = this:current_lag();
        if (lag > 1)
          lagstr = tostr("approximately ", lag, " seconds");
        elseif (lag == 1)
          lagstr = "approximately 1 second";
        else
          lagstr = "low";
        endif
        notify(player, tostr("The lag is ", lagstr, "; there ", (l = length(connected_players())) == 1 ? "is " | "are ", l, " connected."));
      endif
    endif
  endverb

  verb current_lag (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return this.current_lag;
  endverb

  verb maybe_limit_commands (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "This limits the number of commands that can be issued from the login prompt to prevent haywire login programs from lagging the MOO.";
    "$login.current_connections has the current player id's of people at the login prompt.";
    "$login.current_numcommands has the number of commands they have issued at the prompt so far.";
    "$login.max_numcommands has the maximum number of commands they may try before being booted.";
    if (!caller_perms().wizard)
      return E_PERM;
    else
      if (iconn = player in this.current_connections)
        knocks = this.current_numcommands[iconn] = this.current_numcommands[iconn] + 1;
      else
        this.current_connections = {@this.current_connections, player};
        this.current_numcommands = {@this.current_numcommands, 1};
        knocks = 1;
        "...sweep idle connections...";
        for p in (this.current_connections)
          if (typeof(`idle_seconds(p) ! ANY') == ERR)
            n = p in this.current_connections;
            this.current_connections = listdelete(this.current_connections, n);
            this.current_numcommands = listdelete(this.current_numcommands, n);
          endif
        endfor
      endif
      if (knocks > this.max_numcommands)
        notify(player, "Sorry, too many commands issued without connecting.");
        boot_player(player);
        return 1;
      else
        return 0;
      endif
    endif
  endverb

  verb server_started (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Called by #0:server_started when the server restarts.";
    if (caller_perms().wizard)
      this.lag_samples = {0, 0, 0, 0, 0};
      this.downtimes = {{time(), this.last_lag_sample}, @(this.downtimes)[1..min($, 100)]};
      this.intercepted_players = this.intercepted_actions = {};
      this.checkpoint_in_progress = 0;
    endif
  endverb

  verb uptime_since (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "uptime_since(time): How much time has LambdaMOO been up since `time'";
    since = args[1];
    up = time() - since;
    for x in (this.downtimes)
      if (x[1] < since)
        "downtime predates when we're asking about";
        return up;
      endif
      "since the server was down between x[2] and x[1], don't count it as uptime";
      up = up - (x[1] - max(x[2], since));
    endfor
    return up;
  endverb

  verb count_bg_players (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    caller_perms().wizard || $error:raise(E_PERM);
    now = time();
    tasks = queued_tasks();
    sum = 0;
    for t in (tasks)
      delay = t[2] - now;
      interval = delay <= 0 ? 1 | delay * 2;
      "SUM is measured in hundredths of a player for the moment...";
      delay <= 300 && (sum = sum + 2000 / interval);
    endfor
    count = sum / 100;
    return count;
  endverb

  verb "blacklisted_temp graylisted_temp redlisted_temp spooflisted_temp" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":blacklisted_temp(hostname) => is hostname on the .blacklist...";
    ":graylisted_temp(hostname)  => is hostname on the .graylist...";
    ":redlisted_temp(hostname)   => is hostname on the .redlist...";
    ":spooflisted_temp(hostname) => is hostname on the .spooflist...";
    "";
    "... and the time limit hasn't run out.";
    lname = this:listname(verb);
    sitelist = this.(("temporary_" + lname));
    if (!caller_perms().wizard)
      return E_PERM;
    elseif (entry = $list_utils:assoc(hostname = args[1], sitelist[1]))
      return this:templist_expired(lname, @entry);
    elseif (entry = $list_utils:assoc(hostname, sitelist[2]))
      return this:templist_expired(lname, @entry);
    elseif ($site_db:domain_literal(hostname))
      for lit in (sitelist[1])
        if (index(hostname, lit[1]) == 1 && (hostname + ".")[length(lit[1]) + 1] == ".")
          return this:templist_expired(lname, @lit);
        endif
      endfor
    else
      for dom in (sitelist[2])
        if (index(dom[1], "*"))
          "...we have a wildcard; let :match_string deal with it...";
          if ($string_utils:match_string(hostname, dom[1]))
            return this:templist_expired(lname, @dom);
          endif
        else
          "...tail of hostname ...";
          if ((r = rindex(hostname, dom[1])) && (("." + hostname)[r] == "." && r - 1 + length(dom[1]) == length(hostname)))
            return this:templist_expired(lname, @dom);
          endif
        endif
      endfor
    endif
    return 0;
  endverb

  verb templist_expired (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "check to see if duration has expired on temporary_<colorlist>. Removes entry if so, returns true if still <colorlisted>";
    ":(listname, hostname, start time, duration)";
    {lname, hname, start, duration} = args;
    if (!caller_perms().wizard)
      return E_PERM;
    endif
    if (this:uptime_since(start) > duration)
      this:((lname + "_remove_temp"))(hname);
      return 0;
    else
      return 1;
    endif
  endverb

  verb temp_newt_registration_string (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return "Your character is unavailable for another " + $time_utils:english_time(args[1]) + ".";
  endverb

  verb add_interception (this none this) owner: HACKER flags: "rxd"
    caller == this || raise(E_PERM);
    {who, verbname, @arguments} = args;
    who in this.intercepted_players && raise(E_INVARG, "Player already has an interception set.");
    this.intercepted_players = {@this.intercepted_players, who};
    this.intercepted_actions = {@this.intercepted_actions, {verbname, @arguments}};
    return 1;
  endverb

  verb delete_interception (this none this) owner: HACKER flags: "rxd"
    caller == this || raise(E_PERM);
    {who} = args;
    if (loc = who in this.intercepted_players)
      this.intercepted_players = listdelete(this.intercepted_players, loc);
      this.intercepted_actions = listdelete(this.intercepted_actions, loc);
      return 1;
    else
      "raise an error?  nah.";
      return 0;
    endif
  endverb

  verb interception (this none this) owner: HACKER flags: "rxd"
    caller == this || raise(E_PERM);
    {who} = args;
    return (loc = who in this.intercepted_players) ? this.intercepted_actions[loc] | 0;
  endverb

  verb intercepted_password (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    caller == #0 || raise(E_PERM);
    this:delete_interception(player);
    set_connection_option(player, "client-echo", 1);
    notify(player, "");
    try
      {candidate, ?password = ""} = args;
    except (E_ARGS)
      return 0;
    endtry
    return this:connect(tostr(candidate), password);
  endverb

  verb "do_out_of_band_command doobc" (this none this) owner: HACKER flags: "rxd"
    "This is where oob handlers need to be put to handle oob commands issued prior to assigning a connection to a player object.  Right now it simply returns.";
    return;
  endverb

  verb check_for_checkpoint (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (this.checkpoint_in_progress)
      line = "***************************************************************************";
      notify(player, "");
      notify(player, "");
      notify(player, line);
      notify(player, line);
      notify(player, "****");
      notify(player, "****  NOTICE:  The server is very slow now.");
      notify(player, "****           The database is being saved to disk.");
      notify(player, "****");
      notify(player, line);
      notify(player, line);
      notify(player, "");
      notify(player, "");
    endif
  endverb
endobject
