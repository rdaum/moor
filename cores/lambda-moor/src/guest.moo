object GUEST
  name: "Generic Guest"
  parent: FRAND_CLASS
  owner: HACKER
  readable: true

  property default_description (owner: HACKER, flags: "r") = {"By definition, guests appear nondescript."};
  property default_gender (owner: HACKER, flags: "r") = "neuter";
  property extra_confunc_msg (owner: HACKER, flags: "rc") = "";
  property free_to_use (owner: HACKER, flags: "r") = 1;
  property request (owner: BYTE_QUOTA_UTILS_WORKING, flags: "") = 0;

  override aliases (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {"Generic Guest"};
  override description = {"By definition, guests appear nondescript."};
  override features = {PASTING_FEATURE, STAGE_TALK};
  override linelen = 79;
  override lines = 30;
  override mail_forward = "%t (%[#t]) is a guest character.";
  override mail_notify (owner: HACKER, flags: "rc");
  override object_size = {12606, 1084848672};
  override paranoid = 1;
  override password = 0;
  override size_quota = {0, 0, 0, 0};

  verb boot (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return;
    endif
    player = this;
    this:notify(tostr("Sorry, but you've been here for ", $string_utils:from_seconds(connected_seconds(this)), " and someone else wants to be a guest now.  Feel free to come back", @$login:player_creation_enabled(player) ? {" or even create your own character if you want..."} | {" or type `create' to learn more about how to get a character of your own."}));
    "boot_player(this)";
    return;
    "See #0:user_reconnected.";
  endverb

  verb disfunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (valid(cp = caller_perms()) && caller != this && !$perm_utils:controls(cp, this) && cp != this && caller != #0)
      return E_PERM;
    endif
    "Don't let another guest use this one until all this is done. See :defer, Ho_Yan 1/19/94";
    this.free_to_use = 0;
    this:log_disconnect();
    this:erase_paranoid_data();
    try
      if (this.location != this.home)
        this:room_announce(player.name, " has disconnected.");
        this:room_announce($string_utils:pronoun_sub($housekeeper.take_away_msg, this, $housekeeper));
        move(this, this.home);
        this:room_announce($string_utils:pronoun_sub($housekeeper.drop_off_msg, this, $housekeeper));
      endif
    finally
      this:do_reset();
      this.free_to_use = 1;
    endtry
  endverb

  verb defer (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Called by #0:connect_player when this object is about to be used as the next guest character.  Usually returns `this', but if for some reason some other guest character should be used, that player object is returned instead";
    if (!caller_perms().wizard)
      "...caller is not :do_login_command; doesn't matter what we return...";
      return this;
    elseif ($login:blacklisted($string_utils:connection_hostname(connection_name(player))))
      return #-2;
    elseif (!(this in connected_players()))
      "...not logged in, no problemo...";
      return this;
    endif
    longest = 900;
    "...guests get 15 minutes before they can be dislodged...";
    candidate = #-1;
    free = {};
    for g in ($object_utils:leaves($guest))
      if (!is_player(g))
        "...a toaded guest?...";
      elseif (!(con = g in connected_players()) && g.free_to_use)
        "...yay; found an unused guest...and their last :disfunc is complete";
        free = {@free, g};
      elseif (con && (t = connected_seconds(g)) > longest)
        longest = t;
        candidate = g;
      endif
    endfor
    if (free)
      candidate = free[random($)];
    elseif (valid(candidate))
      "...someone's getting bumped...";
      candidate:boot();
    endif
    return candidate;
  endverb

  verb mail_catch_up (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return;
  endverb

  verb create (any any any) owner: HACKER flags: "rd"
    if ($login:player_creation_enabled(player))
      player:tell("First @quit, then connect to the MOO again and, rather than doing `connect guest' do `create <name> <password>'");
    else
      player:tell($login:registration_string());
    endif
  endverb

  verb eject (this none this) owner: HACKER flags: "rxd"
    return pass(@args);
  endverb

  verb log (this none this) owner: HACKER flags: "rxd"
    ":log(islogin,time,where) adds an entry to the connection log for this guest.";
    if (caller != this)
      return E_PERM;
    elseif (length(this.connect_log) < this.max_connect_log)
      this.connect_log = {args, @this.connect_log};
    else
      this.connect_log = {args, @(this.connect_log)[1..this.max_connect_log - 1]};
    endif
  endverb

  verb confunc (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (valid(cp = caller_perms()) && caller != this && !$perm_utils:controls(cp, this) && cp != this && caller != #0)
      return E_PERM;
    else
      $guest_log:enter(1, time(), $string_utils:connection_hostname(connection_name(this)));
      ret = pass(@args);
      this:tell_lines(this:extra_confunc_msg());
      return ret;
    endif
  endverb

  verb log_disconnect (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller != this)
      return E_PERM;
    else
      cname = `connection_name(this) ! ANY' || this.last_connect_place;
      $guest_log:enter(0, time(), $string_utils:connection_hostname(cname));
    endif
  endverb

  verb "@last-c*onnection" (any none none) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!valid(caller_perms()))
      player:tell("Sorry, that information is not available.");
    endif
  endverb

  verb my_huh (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms() != this)
      return E_PERM;
    else
      return pass(@args);
    endif
  endverb

  verb "@read @peek" (any any any) owner: HACKER flags: "rd"
    return pass(@args);
  endverb

  verb set_current_folder (this none this) owner: HACKER flags: "rxd"
    return pass(@args);
    "only for setting permission";
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.extra_confunc_msg = "";
    endif
  endverb

  verb "set_name set_aliases" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "disallow guests from setting aliases on themselves";
    if ($perm_utils:controls(caller_perms(), this))
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb extra_confunc_msg (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    return $string_utils:pronoun_sub(this.(verb));
  endverb

  verb do_reset (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      flush_input(this, 0);
      for x in ({"paranoid", "lines", "responsible", "linelen", "linebuffer", "brief", "gaglist", "rooms", "pagelen", "current_message", "current_folder", "messages", "messages_going", "request", "mail_options", "edit_options", "home", "spurned_objects", "web_info"})
        if ($object_utils:has_property(parent(this), x))
          clear_property(this, x);
        endif
      endfor
      this:set_description(this.default_description);
      this:set_gender(this.default_gender);
      for x in (this.contents)
        this:eject(x);
      endfor
      for x in (this.features)
        if (!(x in $guest.features))
          this:remove_feature(x);
        endif
      endfor
      for x in ($guest.features)
        if (!(x in this.features))
          this:add_feature(x);
        endif
      endfor
      for x in ($object_utils:descendants($generic_editor))
        if (loc = this in x.active)
          x:kill_session(loc);
        endif
      endfor
    endif
  endverb

  verb "@request" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    "Usage:  @request <player-name> for <email-address>";
    if (player != this)
      return player:tell(E_PERM);
    endif
    if (this.request)
      return player:tell("Sorry, you appear to have already requested a character.");
    endif
    name = dobjstr;
    if (prepstr != "for" || (!dobjstr || index(address = iobjstr, " ")))
      return player:notify_lines($code_utils:verb_usage());
    endif
    if ($login:request_character(player, name, address))
      this.request = 1;
    endif
    "Copied from Generic Guest (#5678):@request by Froxx (#49853) Mon Apr  4 10:49:26 1994 PDT";
  endverb

  verb connection_name_hash (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "Compute an encrypted hash of the guest's (last) connection, using 'crypt'. Basically, you can't tell where the guest came from, but it is unlikely that two guests will have the same hash";
    "You can use guest:connection_name_hash(seed) as a string to identify whether two guests are from the same place.";
    hash = toint(caller_perms());
    host = $string_utils:connection_hostname(this.last_connect_place);
    for i in [1..length(host)]
      hash = hash * 14 + index($string_utils.ascii, host[i]);
    endfor
    return crypt(tostr(hash), @args);
  endverb

  verb "@subscribe*-quick @unsubscribed*-quick" (any any any) owner: BYTE_QUOTA_UTILS_WORKING flags: "rd"
    if (caller_perms() != $nothing && caller_perms() != player)
      return E_PERM;
    endif
    if (!args)
      all_mlists = {@$mail_agent.contents, @this.mail_lists};
      if (length(all_mlists) > 50 && !$command_utils:yes_or_no(tostr("There are ", length(all_mlists), " mailing lists.  Are you sure you want the whole list?")))
        return player:tell("OK, aborting.");
      endif
      for c in (all_mlists)
        $command_utils:suspend_if_needed(0);
        if (c:is_usable_by(this) || c:is_readable_by(this) && verb != "@unsubscribed")
          `c:look_self(1) ! ANY';
        endif
      endfor
      player:tell("--End of List--");
    else
      player:tell("Sorry, Guests don't have full mailing privileges.  You may use @read and @peek for mailing lists.  Or try @request to get yourself a character.");
    endif
    "Paragraph (#122534) - Tue Nov 8, 2005 - Added to prevent a silly traceback from occuring, since Guests can't read their own .current_message.";
  endverb

  verb current_folder (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms() in {this, this.owner})
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb

  verb notify (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard || caller_perms() in {this, this.owner} || caller == this)
      return pass(@args);
    else
      return E_PERM;
    endif
  endverb
endobject