object PLAYER [
  import_export_id -> "player"
]
  name: "Generic Player"
  parent: EVENT_RECEIVER
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property admin_features (owner: ARCH_WIZARD, flags: "") = {};
  property llm_assist_contexts (owner: ARCH_WIZARD, flags: "") = {};
  property llm_suggestions (owner: ARCH_WIZARD, flags: "") = {};
  property authoring_features (owner: ARCH_WIZARD, flags: "") = #-1;
  property direct_messages (owner: ARCH_WIZARD, flags: "c") = {};
  property editing_sessions (owner: ARCH_WIZARD, flags: "c") = [];
  property email_address (owner: ARCH_WIZARD, flags: "") = "c";
  property features (owner: ARCH_WIZARD, flags: "rc") = {SOCIAL_FEATURES, MAIL_FEATURES};
  property gaglist (owner: ARCH_WIZARD, flags: "rc") = {};
  property grants_area (owner: ARCH_WIZARD, flags: "") = [];
  property grants_object (owner: ARCH_WIZARD, flags: "") = [];
  property grants_room (owner: ARCH_WIZARD, flags: "") = [];
  property home (owner: ARCH_WIZARD, flags: "rc") = 0;
  property is_builder (owner: ARCH_WIZARD, flags: "") = false;
  property last_connected (owner: ARCH_WIZARD, flags: "r") = 0;
  property last_disconnected (owner: ARCH_WIZARD, flags: "r") = 0;
  property last_dm_from (owner: ARCH_WIZARD, flags: "c") = #-1;
  property oauth2_identities (owner: ARCH_WIZARD, flags: "") = {};
  property object_gaglist (owner: ARCH_WIZARD, flags: "rc") = {};
  property password (owner: ARCH_WIZARD, flags: "c") clear;
  property profile_picture (owner: ARCH_WIZARD, flags: "rc") = false;
  property suggestions_llm_client (owner: ARCH_WIZARD, flags: "") = 0;

  override description (owner: ARCH_WIZARD, flags: "rc") = "You see a player who should get around to describing themself.";

  verb "go" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Go through a local exit by name or to its destination. Usage: go [to] NAME.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    let target = argstr:trim();
    const words = target:raw_words();
    if (length(words) && words[1] == "to")
      target = target:first_word()[2];
    endif
    if (!length(target))
      return this:inform_current($event:mk_info(this,
        "Go where? Type 'exits' to see the ways out."));
    endif
    try
      valid(this.location) && isa(this.location, $room)
        || raise(E_INVARG, "There are no ways out here.");
      return this.location:go(this, target);
    except error (E_INVARG)
      return this:inform_current($event:mk_info(this, error[2]));
    endtry
  endverb

  verb "l*ook" (any none none) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["arguments" -> ["dobj" -> "Object"], "prefix" -> "Look at"]
  ]
    "Look at an object or passage direction.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    if (dobjstr == "")
      target = player.location;
    else
      "Try matching as object first";
      try
        match_result = $match:match_object(dobjstr, player);
      except e (E_INVARG)
        match_result = E_NONE;
      except e (ANY)
        return this:inform_current($event:mk_error(player, "Object lookup failed: " + toliteral(e[2])):with_audience('utility));
      endtry
      if (match_result == $ambiguous_match)
        candidates = {};
        search_space = {player};
        if (valid(player.location))
          search_space = {@search_space, player.location, @player.location.contents};
        endif
        search_space = {@search_space, @player.contents};
        for obj in (search_space)
          if (!valid(obj))
            continue;
          endif
          n = `obj:name() ! ANY => ""';
          if (!n)
            continue;
          endif
          if (index(n, dobjstr) || index(dobjstr, n))
            label = n + " (" + tostr(obj) + ")";
            if (!(label in candidates))
              candidates = {@candidates, label};
            endif
            continue;
          endif
          al = `obj:aliases() ! ANY => {}';
          for a in (al)
            if (typeof(a) == TYPE_STR && a && (index(a, dobjstr) || index(dobjstr, a)))
              label = n + " (" + tostr(obj) + ")";
              if (!(label in candidates))
                candidates = {@candidates, label};
              endif
              break;
            endif
          endfor
        endfor
        if (length(candidates) > 0)
          max = length(candidates) < 5 ? length(candidates) | 5;
          return this:inform_current($event:mk_error(player, "\"" + dobjstr + "\" is ambiguous. Try: " + candidates[1..max]:join(", ") + "."):with_audience('utility));
        endif
        return this:inform_current($event:mk_error(player, "\"" + dobjstr + "\" is ambiguous. Try being more specific."):with_audience('utility));
      elseif (typeof(match_result) == TYPE_ERR || match_result == $failed_match || !valid(match_result))
        "Object match failed - try as passage direction";
        try
          passage_desc = this:_look_passage(dobjstr);
        except e (E_INVARG)
          return this:inform_current($event:mk_error(player, e[2]):with_audience('utility));
        except e (ANY)
          return this:inform_current($event:mk_error(player, "Passage lookup failed: " + toliteral(e)):with_audience('utility));
        endtry
        if (passage_desc)
          return this:inform_current($event:mk_info(player, passage_desc):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
        else
          return this:inform_current($event:mk_not_found(player, "No object or passage found matching '" + dobjstr + "'."):with_audience('utility));
        endif
      else
        target = match_result;
      endif
    endif
    !valid(target) && return this:inform_current(this:msg_no_dobj_match());
    if (target == player.location && isa(target, $room))
      return player:emit_room_look(target);
    endif
    look_d = target:look_self();
    player:inform_current($look:narrative(look_d:capture(player)):with_audience('utility));
  endverb

  method _look_passage owner: ARCH_WIZARD
    "Look at a visible passage or provider exit. Return description content or false.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const {direction} = args;
    const room = this.location;
    valid(room) && isa(room, $room) || return false;
    try
      const option = $room:match_exit(room, this, direction);
      length(option) || return false;
      const description = option['description];
      return length(description) ? description | "You see a way out: " + option['caption] + ".";
    except error (E_PROPNF, E_TYPE, E_INVIND)
      raise(E_INVARG, "Malformed exit in " + tostr(room) + ": " + toliteral(error[1..2]));
    endtry
  endmethod

  verb "i*nventory" (none none none) owner: ARCH_WIZARD flags: "rd"
    "Display player's inventory using list format";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    caller != player && return E_PERM;
    items = this.contents;
    !items && return this:inform_current($event:mk_inventory(player, "You are not carrying anything."):with_audience('utility):with_group('inventory));
    "Get item names";
    item_names = { $sub:reference(item, 'listing) for item in (items) };
    "Create and display the inventory list";
    list_obj = $format.list:mk(item_names);
    title_obj = $format.title:mk("Inventory");
    content = $format.block:mk(title_obj, list_obj);
    event = $event:mk_inventory(player, content);
    this:inform_current(event:with_audience('utility):with_group('inventory));
  endverb

  verb "who @who" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Display connected players, or details for a specific player.";
    caller != player && return E_PERM;
    query = length(args) > 0 ? args:join(" "):trim() | "";
    connected = connected_players();
    if (!connected)
      connected = {};
    endif
    show_ids = player.is_builder || player.programmer;
    if (length(query) > 0)
      try
        matched = $match:match_player(query, this);
      except e (E_INVARG)
        return this:inform_current($event:mk_not_found(this, "I can't find a player named '" + query + "'."):with_audience('utility):with_group('who));
      except e (E_AMBIG)
        return this:inform_current($event:mk_error(this, "That name matches more than one player. Be more specific."):with_audience('utility):with_group('who));
      endtry
      display_name = show_ids ? matched:name() + " (" + tostr(matched) + ")" | matched:name();
      display_name = $format.annotation:object(matched, display_name);
      if (matched in connected)
        headers = {"Name", "Location", "Idle", "Connected"};
        idle_str = idle_seconds(matched):format_time_seconds();
        conn_str = connected_seconds(matched):format_time_seconds();
        if (valid(matched.location))
          location_name = $format.annotation:object(matched.location, show_ids ? matched.location:name() + " (" + tostr(matched.location) + ")" | matched.location:name());
        else
          location_name = "(nowhere)";
        endif
        table_obj = $format.table:mk(headers, {{display_name, location_name, idle_str, conn_str}});
        title_obj = $format.title:mk("Who's Online");
        content = $format.block:mk(title_obj, table_obj);
        return this:inform_current($event:mk_who(player, content):with_audience('utility):with_group('who));
      endif
      last_activity = `matched.last_disconnected ! E_PROPNF => 0';
      if (typeof(last_activity) != TYPE_INT || last_activity <= 0)
        last_activity = `matched.last_connected ! E_PROPNF => 0';
      endif
      if (typeof(last_activity) == TYPE_INT && last_activity > 0)
        elapsed = time() - last_activity;
        elapsed < 0 && (elapsed = 0);
        activity_str = elapsed:format_time_seconds() + " ago";
      else
        activity_str = "unknown";
      endif
      headers = {"Name", "Status", "Last activity"};
      table_obj = $format.table:mk(headers, {{display_name, "offline", activity_str}});
      title_obj = $format.title:mk("Player Status");
      content = $format.block:mk(title_obj, table_obj);
      return this:inform_current($event:mk_who(player, content):with_audience('utility):with_group('who));
    endif
    !connected && return this:inform_current($event:mk_not_found(this, "No players are currently connected."):with_audience('utility):with_group('who));
    "Build table data.";
    headers = {"Name", "Location", "Idle", "Connected"};
    rows = {};
    for p in (connected)
      if (typeof(idle_time = idle_seconds(p)) != TYPE_ERR)
        display_name = $format.annotation:object(p, show_ids ? p:name() + " (" + tostr(p) + ")" | p:name());
        idle_str = idle_time:format_time_seconds();
        conn_str = connected_seconds(p):format_time_seconds();
        if (valid(p.location))
          location_name = $format.annotation:object(p.location, show_ids ? p.location:name() + " (" + tostr(p.location) + ")" | p.location:name());
        else
          location_name = "(nowhere)";
        endif
        rows = {@rows, {p:name(), {display_name, location_name, idle_str, conn_str}}};
      endif
    endfor
    "Sort by player name for stable output.";
    sorted = {};
    for entry in (rows)
      inserted = false;
      for i in [1..length(sorted)]
        if (entry[1] < sorted[i][1])
          sorted = {@sorted[1..i - 1], entry, @sorted[i..$]};
          inserted = true;
          break;
        endif
      endfor
      !inserted && (sorted = {@sorted, entry});
    endfor
    table_rows = {};
    for entry in (sorted)
      table_rows = {@table_rows, entry[2]};
    endfor
    "Create and display the table.";
    if (table_rows)
      table_obj = $format.table:mk(headers, table_rows);
      title_obj = $format.title:mk("Who's Online");
      content = $format.block:mk(title_obj, table_obj);
      event = $event:mk_who(player, content);
      this:inform_current(event:with_audience('utility):with_group('who));
    else
      this:inform_current($event:mk_who(this, "No connected players found."):with_audience('utility):with_group('who));
    endif
  endverb

  method "msg_no_dobj_match msg_no_iobj_match" owner: ARCH_WIZARD
    "Utility verb to produce a not found event for presenting to the player...";
    set_task_perms(this);
    return $event:mk_not_found(player, "I don't see that here."):with_audience('utility);
  endmethod

  verb "@describe" (this as any) owner: ARCH_WIZARD flags: "rd"
    "Describe yourself: @describe me as TEXT. Builders use the same command for objects and exits.";
    "Only `me` matches this argument form on your own player; other targets reach builder features.";
    if (this != player)
      player:inform_current($event:mk_error(player, "You can only describe yourself."));
      return;
    endif
    set_task_perms(this);
    const text = argstr:raw_arguments()[2];
    if (!length(text:trim()))
      this:inform_current($event:mk_error(this, $format.code:mk("@describe me as DESCRIPTION")));
      return;
    endif
    try
      this:set_description(text);
    except e (E_INVARG, E_TYPE, E_PERM)
      this:inform_current($event:mk_error(this, typeof(e[2]) == TYPE_STR ? e[2] | toliteral(e)));
      return;
    endtry
    this:inform_current($event:mk_info(this, "Your description is set."));
  endverb

  verb "@pronouns" (any any any) owner: ARCH_WIZARD flags: "rd"
    "View your pronouns, choose a preset, or enter custom words with a preview.";
    "Usage: @pronouns [PRESET|custom]. Custom entry commits at each prompt and rechecks access on save.";
    caller == this || raise(E_PERM);
    set_task_perms(this);
    const query = argstr:trim();
    if (!length(query))
      const lines = {"Your pronouns: " + this:pronouns_display(),
        "Presets: " + $pronouns:list_presets():join(", "),
        "Use @pronouns PRESET, or @pronouns custom to enter your own words."};
      this:inform_current($event:mk_info(this, @{$format.paragraph:mk(line) for line in (lines)})
        :with_audience('utility));
      return;
    endif
    try
      let input = query;
      if (query == "custom")
        input = $pronouns:prompt_custom(this);
        typeof(input) == TYPE_MAP || return;
      endif
      this:set_pronouns(input);
      this:inform_current($event:mk_info(this, "Pronouns set to: " + this:pronouns_display())
        :with_audience('utility));
    except error (E_INVARG, E_TYPE, E_PERM)
      this:inform_current($event:mk_error(this, error[2]):with_audience('utility));
    endtry
  endverb

  method acceptable owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return !is_player(args[1]);
  endmethod

  method profile_picture owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return this.profile_picture;
  endmethod

  method thumbnail owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    "Return thumbnail (profile) image data for this player.";
    return this.profile_picture;
  endmethod

  method set_profile_picture owner: ARCH_WIZARD
    "Update the profile picture of the given player.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_profile_picture);
    set_task_perms(perms);
    const {content_type, picbin} = args;
    length(picbin) > 5 * (1 << 23) && raise(E_INVARG("Profile picture too large"));
    typeof(content_type) == TYPE_STR && content_type:starts_with("image/") || raise(E_TYPE);
    typeof(picbin) == TYPE_BINARY || raise(E_TYPE);
    target.profile_picture = {content_type, picbin};
  endmethod

  method set_password owner: ARCH_WIZARD
    "Change this player's password. Permission: wizard, owner, or 'set_password capability.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_password);
    set_task_perms(perms);
    const {new_password} = args;
    target.password = $password:mk(new_password);
  endmethod

  verb "@password" (any any any) owner: ARCH_WIZARD flags: "rd"
    "Change your password. Usage: @password <old-password> <new-password>";
    caller != this && raise(E_PERM);
    set_task_perms(this);
    "If password not set, only need new password";
    if (typeof(this.password) != TYPE_FLYWEIGHT)
      if (length(args) != 1)
        return this:inform_current($event:mk_error(this, $format.code:mk("Usage: @password <new-password>")):with_audience('utility));
      endif
      new_password = args[1];
    elseif (length(args) != 2)
      this:inform_current($event:mk_error(this, $format.code:mk("Usage: @password <old-password> <new-password>")):with_audience('utility));
      return;
    elseif (!this.password:challenge(tostr(args[1])))
      this:inform_current($event:mk_error(this, "That's not your old password."):with_audience('utility));
      return;
    else
      new_password = args[2];
    endif
    "Set the new password";
    this.password = $password:mk(tostr(new_password));
    this:inform_current($event:mk_info(this, "New password set."):with_audience('utility));
  endverb

  method set_player_flag owner: ARCH_WIZARD
    "Mark this object as a player. Permission: wizard or 'set_player_flag capability.";
    const {flag_value} = args;
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_player_flag);
    set_task_perms(perms);
    set_player_flag(target, flag_value);
  endmethod

  method set_programmer owner: ARCH_WIZARD
    "Set this player's programmer flag. Permission: wizard, owner, or 'set_programmer capability.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_programmer);
    set_task_perms(perms);
    const {flag_value} = args;
    target.programmer = flag_value;
  endmethod

  method set_email_address owner: ARCH_WIZARD
    "Set this player's email address. Permission: wizard, owner, or 'set_email_address capability.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_email_address);
    set_task_perms(perms);
    const {email} = args;
    target.email_address = email;
  endmethod

  method set_oauth2_identities owner: ARCH_WIZARD
    "Administrative identity repair requires a wizard or an explicit wizard-issued capability.";
    "Verify claims here: a player-owned delegate can override capability-checking methods.";
    let target = this;
    if (typeof(this) == TYPE_FLYWEIGHT)
      const claims = paseto_verify_local(this.token);
      target = this.delegate;
      claims["target"] == target && 'set_oauth2_identities in claims["caps"] || raise(E_PERM);
      maphaskey(claims, "exp") && time() > claims["exp"] && raise(E_PERM);
      $root:_capability_is_revoked(claims) && raise(E_PERM);
      const issuer = claims["granted_by"];
      const authority = `claims["run_as"] ! E_RANGE => $nothing';
      valid(issuer) && issuer.wizard && valid(authority) && authority.wizard || raise(E_PERM);
    else
      caller_perms().wizard || raise(E_PERM);
    endif
    const {identities} = args;
    target.oauth2_identities = identities;
  endmethod

  method look_self owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    base_desc = this.description;
    if (!(this in connected_players()))
      description = {base_desc, " ", $sub:sc_dobj(), " ", $sub:verb_be_dobj(), " sleeping."};
    elseif ((idle = idle_seconds(this)) < 60)
      description = {base_desc, " ", $sub:sc_dobj(), " ", $sub:verb_be_dobj(), " awake and ", $sub:verb_look_dobj(), " alert."};
    else
      time = $str_proto:from_seconds(idle);
      description = {base_desc, " ", $sub:sc_dobj(), " ", $sub:verb_be_dobj(), " awake, but ", $sub:verb_have_dobj(), " been staring off into space for ", time, "."};
    endif
    "Append shared actor details from $actor.";
    details = this:_look_self_details();
    if (details && length(details) > 0)
      description = {@description, @details};
    endif
    "Don't show inventory contents when looking at a player - that's private";
    return <$look, .what = this, .title = this:name(), .description = description>;
  endmethod

  method match_environment owner: ARCH_WIZARD
    "Return parser scope entries. Context may narrow to inventory or expand into a target's visible contents.";
    caller_perms() == this || caller == #0 || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this);
    const {?command = "", ?context = []} = args;
    const scope = `context['scope] ! E_RANGE => "nearby"';
    const target = `context['target] ! E_RANGE => $nothing';
    if (scope == "inventory")
      return { item for item in (this:contents()) if item != target };
    elseif (scope == "contents")
      valid(target) && (target.location == this || $thing:take_reachable(target, this)) || return {};
      return target:match_scope_for(this, context);
    endif
    scope == "nearby" || raise(E_INVARG, "Unknown matching scope");
    const carried = { carried_item for carried_item in (this:contents()) if valid(carried_item) };
    const worn = { worn_item for worn_item in (this.wearing) if valid(worn_item) };
    let env = {this, @carried, @worn};
    const mailbox = this:find_mailbox();
    valid(mailbox) && (env = {@env, mailbox});
    const location = this.location;
    if (valid(location))
      const ambient = location:match_scope_for(this, context);
      env = {@env, @ambient, location};
    endif
    return env;
  endmethod

  method command_environment owner: ARCH_WIZARD
    "Return the player, features, and location searched for ambient command verbs.";
    caller_perms() == this || caller == #0 || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this);
    let env = {this, @this:_feature_environment()};
    const location = this.location;
    valid(location) && (env = {@env, location});
    return env;
  endmethod

  method _feature_environment owner: ARCH_WIZARD
    "Return feature objects that extend this player's command/help surface.";
    caller != this && caller != #0 && caller_perms() != this && !caller_perms().wizard && return E_PERM;
    env = {};
    seen = [];
    features = typeof(this.features) == TYPE_LIST ? this.features | {};
    candidates = {@features, this.authoring_features};
    admin_features = this.admin_features;
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    candidates = {@candidates, @admin_features};
    const wizard = this.wizard;
    const builder = this.is_builder;
    set_task_perms(this);
    wizard || this:has_admin_elevation() && (candidates = {@candidates, $wiz_features});
    builder && (candidates = {@candidates, $builder_features});
    for feat in (candidates)
      if (!valid(feat))
        continue;
      endif
      if (maphaskey(seen, feat))
        continue;
      endif
      env = {@env, feat};
      seen[feat] = true;
    endfor
    return env;
  endmethod

  method _get_grants owner: ARCH_WIZARD
    "Internal: Get grants map for a category. ARCH_WIZARD owned to read private property.";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {category} = args;
    prop_name = "grants_" + tostr(category);
    return `this.(prop_name) ! E_PROPNF => false';
  endmethod

  method find_capability_for owner: ARCH_WIZARD
    "Find a capability token for target_obj in the specified category. Returns token or false.";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this);
    {target_obj, category} = args;
    typeof(target_obj) == TYPE_OBJ || return false;
    typeof(category) == TYPE_SYM || return false;
    "Get the grants map via wizard-owned accessor";
    grants_map = this:_get_grants(category);
    typeof(grants_map) == TYPE_MAP || return false;
    "Check if we have a grant for this specific object";
    if (maphaskey(grants_map, target_obj))
      return grants_map[target_obj];
    endif
    return false;
  endmethod

  method find_mailbox owner: ARCH_WIZARD
    "Find this player's mailbox. Returns the mailbox, or #-1 if none.";
    "Raises E_INVARG if player owns multiple mailboxes.";
    if (caller_perms() != this && !caller_perms().wizard)
      raise(E_PERM, "Can only find your own mailbox");
    endif
    found = {};
    for item in (owned_objects(this))
      if (typeof(item) == TYPE_OBJ && valid(item) && isa(item, $mailbox))
        found = {@found, item};
      endif
    endfor
    if (length(found) > 1)
      raise(E_INVARG, "Player owns multiple mailboxes");
    endif
    return length(found) == 1 ? found[1] | #-1;
  endmethod

  method confunc owner: ARCH_WIZARD
    "Called when player connects. Check for new DMs and unread mail.";
    "last_connected still holds the *previous* connection time here.";
    last_conn = this.last_connected;
    "Check for DMs received since last connection.";
    new_dm_count = 0;
    for dm in (this.direct_messages)
      if (dm.sent > last_conn)
        new_dm_count = new_dm_count + 1;
      endif
    endfor
    if (new_dm_count > 0)
      msg = $format.paragraph:mk(new_dm_count == 1 ? "You have a new direct message. Type "
        | tostr("You have ", new_dm_count, " new direct messages. Type "),
        $format.annotation:command("dms", "dms"), new_dm_count == 1 ? " to read it." | " to read them.");
      event = $event:mk_info(this, msg):as_inset():with_group('dm_notify);
      this:inform_current(event);
    endif
    "Check for unread mail.";
    mailbox = this:find_mailbox();
    if (valid(mailbox))
      unread = mailbox:unread_count();
      if (unread > 0)
        msg = $format.paragraph:mk(unread == 1 ? "You have an unread letter. Type "
          | tostr("You have ", unread, " unread letters. Type "),
          $format.annotation:command("mail", "mail"), " to check your mailbox.");
        event = $event:mk_info(this, msg):as_inset():with_group('mail_notify);
        this:inform_current(event);
      endif
    endif
    "Now update last_connected for next login.";
    this.last_connected = time();
  endmethod

  method is_wearing owner: ARCH_WIZARD
    "Check if player is wearing the specified item.";
    set_task_perms(caller_perms());
    {item} = args;
    wearing_list = this.wearing;
    return typeof(wearing_list) == TYPE_LIST && is_member(item, wearing_list);
  endmethod

  method confirm owner: ARCH_WIZARD
    "Show a confirmation prompt and return true if confirmed, false if cancelled, or string with alternative instruction.";
    "Returns: true (confirmed), false (cancelled/no), or string (alternative feedback)";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {message, ?alt_label = "Or suggest an alternative:", ?alt_placeholder = "Describe your alternative approach...", ?tts_prompt = 0} = args;
    metadata = {{"input_type", "yes_no_alternative"}, {"prompt", message}, {"alternative_label", alt_label}, {"alternative_placeholder", alt_placeholder}};
    typeof(tts_prompt) == TYPE_STR && (metadata = {@metadata, {"tts_prompt", tts_prompt}});
    response = this:read_with_prompt(metadata);
    set_task_perms(this);
    if (response == "yes")
      this:inform_current($event:mk_info(this, "Confirmed."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return true;
    elseif (response == "no")
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    elseif (index(response, "alternative: ") == 1)
      alt_text = response[14..$];
      this:inform_current($event:mk_info(this, "Alternative provided: " + alt_text):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return alt_text;
    else
      "Fallback for unexpected responses";
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
  endmethod

  method prompt owner: HACKER
    "Show an open-ended prompt and return the user's text response.";
    "Returns: string (user's response) or false if cancelled/empty";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {question, ?placeholder = "Enter your response...", ?tts_prompt = 0} = args;
    metadata = {{"input_type", "text_area"}, {"prompt", question}, {"placeholder", placeholder}, {"rows", 4}};
    typeof(tts_prompt) == TYPE_STR && (metadata = {@metadata, {"tts_prompt", tts_prompt}});
    response = this:read_with_prompt(metadata);
    if (response == "@abort" || typeof(response) != TYPE_STR)
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
    text = response:trim();
    if (!text || text == "")
      this:inform_current($event:mk_info(this, "No response provided."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
    return text;
  endmethod

  method upload owner: HACKER
    "Request a file upload from the player.";
    "Args: prompt, ?accept_content_types = {}, ?max_file_size = 0, ?tts_prompt = 0";
    "Returns: {content_type, binary_data} or false if cancelled";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {question, ?accept_content_types = {}, ?max_file_size = 0, ?tts_prompt = 0} = args;
    metadata = {{"input_type", "file"}, {"prompt", question}};
    typeof(accept_content_types) == TYPE_LIST && length(accept_content_types) > 0 && (metadata = {@metadata, {"accept_content_types", accept_content_types}});
    typeof(max_file_size) == TYPE_INT && max_file_size > 0 && (metadata = {@metadata, {"max_file_size", max_file_size}});
    typeof(tts_prompt) == TYPE_STR && (metadata = {@metadata, {"tts_prompt", tts_prompt}});
    response = this:read_with_prompt(metadata);
    if (typeof(response) != TYPE_LIST || length(response) != 2)
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
    {content_type, data} = response;
    if (typeof(content_type) != TYPE_STR || typeof(data) != TYPE_BINARY)
      this:inform_current($event:mk_info(this, "Invalid upload format."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
    return response;
  endmethod

  method upload_image owner: HACKER
    "Request an image upload from the player.";
    "Args: prompt, ?max_file_size = 0, ?tts_prompt = 0";
    "Returns: {content_type, binary_data} or false if cancelled";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {question, ?max_file_size = 0, ?tts_prompt = 0} = args;
    return this:upload(question, {"image/png", "image/jpeg", "image/gif", "image/webp"}, max_file_size, tts_prompt);
  endmethod

  method read_multiline owner: ARCH_WIZARD
    "Request multiline content from the player.";
    "Host (both web and telnet) is expected to handle this via metadata.";
    {?prompt = 0, ?tts_prompt = 0} = args;
    metadata = {{'input_type, "text_area"}};
    if (typeof(prompt) == TYPE_STR && length(prompt))
      metadata = {@metadata, {'prompt, prompt}};
    endif
    typeof(tts_prompt) == TYPE_STR && (metadata = {@metadata, {'tts_prompt, tts_prompt}});
    return this:read_with_prompt(metadata);
  endmethod

  method read_with_prompt owner: ARCH_WIZARD
    "Helper: Call read() with metadata, displaying prompt via notify() for telnet clients.";
    "Args: metadata (list of {key, value} pairs for read())";
    "Returns: result from read()";
    {metadata} = args;
    perms = caller_perms();
    "Allow if caller is wizard, the player, or the task perms already match the player (after set_task_perms)";
    perms.wizard || caller == this || perms == this || raise(E_PERM);
    typeof(metadata) == TYPE_LIST || raise(E_TYPE, "Metadata must be list");
    return read(this, metadata);
  endmethod

  verb put (any in this) owner: ARCH_WIZARD flags: "rd"
    "Reject putting things in a player";
    event = $event:mk_error(player, $sub:tc(), " ", $sub:agree('this, 'name, "are", "is"), " a person, not a container."):with_this(this);
    player:inform_current(event);
  endverb

  method make_player owner: ARCH_WIZARD
    "Create a player and return a setup capability for initial configuration.";
    const actor = caller_perms();
    const {_, perms} = $root:_check_permissions_as(this, actor, 'make_player);
    return this:_make_player_setup_cap(perms);
  endmethod

  method _make_player_setup_cap owner: ARCH_WIZARD
    "Create a player with the authority selected by a wizard-owned caller.";
    caller_perms().wizard || raise(E_PERM);
    const {perms, ?key = 0} = args;
    valid(perms) || raise(E_INVARG);
    set_task_perms(perms);
    const new_player = this:create();
    return $root:issue_capability(new_player, {'set_player_flag, 'set_owner, 'set_name_aliases, 'set_password, 'set_programmer, 'set_email_address, 'set_oauth2_identities, 'set_home, 'move}, 0, perms, key);
  endmethod

  verb "exam*ine x" (any none none) owner: ARCH_WIZARD flags: "rxd" [
    ui -> ["arguments" -> ["dobj" -> "Object"]]
  ]
    "Display detailed information about an object.";
    "Syntax: examine <object>";
    "";
    "Shows the object's name, aliases, owner, description, parent, location, contents, and available verbs.";
    caller == this || caller == #0 || raise(E_PERM);
    set_task_perms(this);
    "Check if called with direct object reference first";
    target = E_NONE;
    if (length(args) > 0 && typeof(args[1]) == TYPE_OBJ && valid(args[1]))
      target = args[1];
    elseif (dobjstr == "")
      return this:inform_current($event:mk_not_found(this, "Examine what?"):with_audience('utility));
    else
      "Try to match the object from dobjstr";
      try
        target = $match:match_object(dobjstr, player);
      except e (E_INVARG)
        return this:inform_current($event:mk_not_found(player, "Could not find '" + dobjstr + "' to examine."):with_audience('utility));
      except e (ANY)
        return this:inform_current($event:mk_error(player, "Object lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
      if (typeof(target) == TYPE_ERR)
        return this:inform_current($event:mk_not_found(player, "No object found matching '" + dobjstr + "'."):with_audience('utility));
      endif
    endif
    !valid(target) && return this:inform_current(this:msg_no_dobj_match());
    "Get the examination flyweight";
    exam = target:examination();
    if (typeof(exam) != TYPE_FLYWEIGHT)
      return this:inform_current($event:mk_error(this, "Could not examine that object."):with_audience('utility));
    endif
    "Build the display output following LambdaCore style";
    lines = {};
    "Header with object name, aliases, and number";
    header_parts = {exam.name};
    if (exam.aliases && length(exam.aliases) > 0)
      header_parts = {@header_parts, "aka " + exam.aliases:join(" and ")};
    endif
    header_parts = {@header_parts, "and", tostr(exam.object_ref)};
    header = header_parts:join(" ");
    lines = {@lines, $format.title:mk($format.annotation:object(exam.object_ref, header))};
    "Ownership";
    if (valid(exam.owner))
      owner_name = `exam.owner:name() ! ANY => tostr(exam.owner)';
      lines = {@lines, $format.paragraph:mk("Owned by ", $format.annotation:object(exam.owner, owner_name), ".")};
    else
      lines = {@lines, "(Unowned)"};
    endif
    "Description";
    if (typeof(exam.description) == TYPE_FLYWEIGHT || exam.description)
      lines = {@lines, exam.description};
    else
      lines = {@lines, "(No description set.)"};
    endif
    "Obvious verbs if any";
    if (exam.verbs && length(exam.verbs) > 0)
      lines = {@lines, ""};
      "Build user-friendly verb signatures using obj_utils";
      const actions = exam.object_ref:inspection_commands(this);
      verb_sigs = { $obj_utils:command_entry(exam.object_ref, spec, this, "", actions) for spec in (exam.verbs) };
      "Create formatted verb list";
      verb_list = $format.list:mk(verb_sigs, false);
      verb_title = $format.title:mk("Obvious verbs");
      lines = {@lines, verb_title, verb_list};
    endif
    "Create formatted block and send as event";
    content = $format.block:mk(@lines);
    event = $event:mk_info(this, content):with_audience('utility):with_presentation_hint('inset):with_group('utility, this);
    this:inform_current(event);
  endverb

  verb "help what" (any any any) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["arguments" -> ["dobj" -> "Topic"], "prefix" -> "Help with"]
  ]
    "Tell the player where they are and what's around.";
    "Display available commands and actions. If a target object is specified, show help for that object.";
    "If a topic name is given, search for help on that topic.";
    "Syntax: help [object|topic]";
    "        help <topic>";
    "        help <topic> from <source>";
    "        help <source_ref>:<topic>";
    "        help <category> <topic>";
    "        help topic <topic>";
    "        help object <ref>";
    "        help source <source>";
    caller != this && raise(E_PERM);
    set_task_perms(this);
    "No argument - show help summary";
    if (!dobjstr || dobjstr == "")
      lines = {};
      "Location context first";
      context_block = `$help_utils:display_location_context(this) ! ANY => 0';
      if (typeof(context_block) == TYPE_FLYWEIGHT)
        lines = {@lines, context_block};
      endif
      "Help topics near the top";
      topics = this:_collect_help_topics();
      if (length(topics) > 0)
        topic_names = {};
        for t in (topics)
          if (!(t.name in topic_names))
            topic_names = {@topic_names, t.name};
          endif
        endfor
        lines = {@lines, "", $format.title:mk("Help Topics", 4)};
        lines = {@lines, $format.list:mk({ $format.annotation:help(topic_entry.provider, topic_entry.name) for topic_entry in (topics) }, false)};
        lines = {@lines, $format.paragraph:mk("Type ", $format.code:inline("help <topic>"), " for details.")};
      endif
      "Then commands";
      cmd_env = this:command_environment();
      ambient_verbs = $obj_utils:collect_ambient_verbs(cmd_env);
      lines = this:_display_ambient_verbs(ambient_verbs, lines);
      if (length(ambient_verbs) == 0)
        lines = {@lines, "(No commands available)"};
      endif
      lines = {@lines, "", $format.title:mk("Need more detail?", 4)};
      lines = {@lines, $format.code:mk("help <thing>\nexamine <thing>")};
      lines = {@lines, "Use these on anything listed above to see its specific commands or description."};
      if (this.programmer)
        lines = {@lines, "", $format.title:mk("To look for programmer documentation...")};
        lines = {@lines, $format.code:mk("@doc object\n@doc object:verb")};
      endif
      content = $format.block:mk($format.title:mk("Help"), @lines);
      event = $event:mk_info(this, content):with_audience('utility):as_inset():with_metadata('collapse_title, "Help"):with_group('help, this);
      this:inform_current(event);
      return;
    endif
    query = dobjstr:trim();
    force_topic = false;
    force_object = false;
    "Allow explicit disambiguation via leading keyword in dobjstr.";
    if (index(query, "topic ") == 1)
      force_topic = true;
      query = query[7..$]:trim();
    elseif (index(query, "object ") == 1)
      force_object = true;
      query = query[8..$]:trim();
    endif
    if (!query)
      return this:inform_current($event:mk_error(this, "Usage: ", $format.code:inline("help [topic|object|source] <name>")):with_audience('utility));
    endif
    "Support: help source (list)";
    if (query in {"source", "sources"})
      return this:_display_help_sources();
    endif
    "Support: help source <source>";
    if (index(query, "source ") == 1)
      source_query = query[8..$]:trim();
      if (!source_query)
        return this:_display_help_sources();
      endif
      env = this:help_environment();
      sources = {};
      for o in (env)
        if (!o.r)
          continue;
        endif
        if (!respond_to(o, 'help_topics))
          continue;
        endif
        name = `o.name ! ANY => ""';
        if (!name)
          continue;
        endif
        sources = {@sources, o};
      endfor
      if (!sources)
        return this:inform_current($event:mk_error(this, "No help sources are available here."):with_audience('utility));
      endif
      src = $match:resolve_in_scope(source_query, sources);
      if (src == $ambiguous_match)
        return this:inform_current($event:mk_error(this, "More than one help source matches. Use a more specific name."));
      endif
      if (src == $failed_match)
        names = {};
        for s in (sources)
          names = {@names, s.name};
        endfor
        content = $format.block:mk($format.title:mk("Unknown Help Source"), $format.paragraph:mk("No help source matches '" + source_query + "'."), $format.list:mk(names), $format.paragraph:mk("Try: ", $format.code:inline("help source <name>")));
        event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
        this:inform_current(event);
        return;
      endif
      try
        topics = src:help_topics(this, "");
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Help topic provider failed: " + toliteral(e)):with_audience('utility));
      endtry
      lines = {"Help topics from " + src.name + ":", ""};
      if (!topics || typeof(topics) != TYPE_LIST)
        lines = {@lines, "(No topics)"};
      else
        const topic_links = { $format.paragraph:mk($format.annotation:help(src, topic_entry.name), " \u2014 ", topic_entry.summary) for topic_entry in (topics) };
        lines = {@lines, $format.list:mk(topic_links, false)};
      endif
      lines = {@lines, $format.paragraph:mk("Tip: ", $format.code:inline("help <topic> from " + src.name))};
      content = $format.block:mk($format.title:mk("Help Source"), @lines);
      event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
      this:inform_current(event);
      return;
    endif
    "Support: help <source_ref>:<topic> (e.g. help $prog_features:@list)";
    parts = query:split(":");
    if (length(parts) >= 2)
      topic_query = parts[$]:trim();
      source_ref = parts[1..$ - 1]:join(":"):trim();
      if (source_ref && topic_query)
        try
          src = $match:match_object(source_ref, this);
        except e (E_INVARG)
          src = $failed_match;
        except e (ANY)
          return this:inform_current($event:mk_error(this, "Help source lookup failed: " + toliteral(e[2])):with_audience('utility));
        endtry
        if (valid(src) && respond_to(src, 'help_topics))
          try
            topic_result = src:help_topics(this, topic_query);
          except e (ANY)
            return this:inform_current($event:mk_error(this, "Help topic provider failed: " + toliteral(e)):with_audience('utility));
          endtry
          if (typeof(topic_result) != TYPE_INT)
            prose_lines = topic_result:render_prose();
            content = $format.block:mk($format.title:mk(topic_result.name), @prose_lines);
            event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
            this:inform_current(event);
            return;
          endif
          return this:inform_current($event:mk_error(this, "No help found for '" + topic_query + "' from " + src.name + "."):with_audience('utility));
        endif
      endif
    endif
    "Heuristic disambiguation:";
    "- '$...' is almost certainly an object reference";
    "- '#<digit/hex>...' is almost certainly an object reference, but '#' alone may be a topic";
    "- '@...' is almost certainly a command/help topic (not a player reference)";
    if (!force_topic && !force_object)
      if (query[1] == "$")
        force_object = true;
      elseif (query[1] == "#" && length(query) > 1)
        "Only treat as object ref if second char is digit or hex (e.g. #123, #0000AB-123)";
        second = query[2];
        if (second >= "0" && second <= "9" || (second >= "a" && second <= "f") || (second >= "A" && second <= "F"))
          force_object = true;
        endif
      elseif (query[1] == "@")
        force_topic = true;
      endif
    endif
    "Detect scoped topic lookup: either via preposition ('from') or inline ' from ...' in dobjstr.";
    source_scope = "";
    if (prepstr == "from" && iobjstr && iobjstr != "")
      source_scope = iobjstr:trim();
    elseif (index(query, " from ") > 0)
      pos = index(query, " from ");
      source_scope = query[pos + 6..$]:trim();
      query = query[1..pos - 1]:trim();
    endif
    "Support: help <category> <topic> (e.g. help programming @list)";
    cat_query = "";
    rest_query = "";
    words = query:split(" ");
    if (length(words) >= 2)
      cat_query = words[1];
      rest_query = words[2..$]:join(" "):trim();
      if (cat_query && rest_query)
        try
          base = this:find_help_topic(rest_query);
        except e (ANY)
          return this:inform_current($event:mk_error(this, "Help topic lookup failed: " + toliteral(e)):with_audience('utility));
        endtry
        if (typeof(base) == TYPE_LIST)
          filtered = {};
          cat_sym = 0;
          try
            cat_sym = tosym(cat_query);
          except e (ANY)
            cat_sym = 0;
          endtry
          for match in (base)
            {source, topic} = match;
            if (typeof(topic) != TYPE_FLYWEIGHT)
              continue;
            endif
            if (cat_sym && topic.category == cat_sym)
              filtered = {@filtered, match};
            endif
          endfor
          if (length(filtered) == 1)
            topic = filtered[1][2];
            prose_lines = topic:render_prose();
            content = $format.block:mk($format.title:mk(topic.name), @prose_lines);
            event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
            this:inform_current(event);
            return;
          endif
        endif
      endif
    endif
    "Topics first (unless user forced object).";
    if (!force_object)
      if (source_scope)
        force_topic = true;
        env = this:help_environment();
        sources = {};
        keys = {};
        for o in (env)
          if (!o.r)
            continue;
          endif
          if (!respond_to(o, 'help_topics))
            continue;
          endif
          name = `o.name ! ANY => ""';
          if (!name)
            continue;
          endif
          sources = {@sources, o};
        endfor
        src = $match:resolve_in_scope(source_scope, sources);
        if (src == $ambiguous_match)
          return this:inform_current($event:mk_error(this, "More than one help source matches. Use a more specific name."));
        endif
        if (src == $failed_match)
          return this:inform_current($event:mk_error(this, "No help source matches '" + source_scope + "'. Try: ", $format.code:inline("help source <source>")):with_audience('utility));
        endif
        try
          topic_result = src:help_topics(this, query);
        except e (ANY)
          return this:inform_current($event:mk_error(this, "Help topic provider failed: " + toliteral(e)):with_audience('utility));
        endtry
        if (typeof(topic_result) != TYPE_INT)
          prose_lines = topic_result:render_prose();
          content = $format.block:mk($format.title:mk(topic_result.name), @prose_lines);
          event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
          this:inform_current(event);
          return;
        endif
        return this:inform_current($event:mk_error(this, "No help found for '" + query + "' from " + src.name + "."):with_audience('utility));
      endif
      try
        topic_result = this:find_help_topic(query);
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Help topic lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
      if (typeof(topic_result) == TYPE_LIST)
        return this:_display_ambiguous_topic_matches(query, topic_result);
      elseif (typeof(topic_result) != TYPE_INT)
        prose_lines = topic_result:render_prose();
        content = $format.block:mk($format.title:mk(topic_result.name), @prose_lines);
        event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
        this:inform_current(event);
        return;
      endif
    endif
    "No topic match (or forced object) - try object help unless forced topic.";
    if (!force_topic)
      target_obj = $failed_match;
      if (force_object || query in {"here", "me", "myself", "player"} || query[1] in {"#", "$"})
        try
          target_obj = $match:match_object(query, this);
        except e (E_INVARG)
          target_obj = $failed_match;
        except e (ANY)
          return this:inform_current($event:mk_error(this, "Help object lookup failed: " + toliteral(e[2])):with_audience('utility));
        endtry
      else
        "Strict-ish match only against direct scope: me, here, room contents, inventory.";
        targets = {};
        if (valid(this))
          targets = {@targets, this};
        endif
        if (valid(this.location))
          targets = {@targets, this.location};
          for item in (this.location.contents)
            if (item != this)
              targets = {@targets, item};
            endif
          endfor
        endif
        for item in (this.contents)
          targets = {@targets, item};
        endfor
        keys = {};
        final_targets = {};
        for obj in (targets)
          if (typeof(obj) != TYPE_OBJ || !valid(obj))
            continue;
          endif
          "Respect readability if available.";
          try
            if (!obj.r)
              continue;
            endif
          except e (ANY)
          endtry
          name = `obj.name ! ANY => ""';
          if (!name)
            continue;
          endif
          final_targets = {@final_targets, obj};
        endfor
        if (final_targets)
          "Help only reads, so take the obvious match: a fixture over its room, one of identical items.";
          const found = $match:prefer($match:candidates_in_scope(query, final_targets, ['context -> this]), this);
          if (length(found) > 1)
            return this:inform_current($event:mk_error(this, "More than one object matches. Use a more specific name."));
          endif
          target_obj = length(found) ? found[1] | $failed_match;
        endif
      endif
      if (valid(target_obj))
        try
          result = this:_show_targeted_help(target_obj);
        except e (ANY)
          return this:inform_current($event:mk_error(this, "Object help lookup failed: " + toliteral(e)):with_audience('utility));
        endtry
        lines = result[1];
        if (this.programmer)
          lines = {@lines, "", $format.title:mk("Try programmer documentation with:")};
          lines = {@lines, $format.code:mk("@doc " + query + "\n@doc " + query + ":verb")};
        endif
        content = $format.block:mk($format.title:mk("Help for " + target_obj:display_name() + "(" + toliteral(target_obj) + ")"), @lines);
        event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
        this:inform_current(event);
        return;
      endif
    endif
    "Neither object nor topic found - try LLM suggestions";
    if (!this:suggest_help_topic(query))
      this:inform_current($event:mk_error(this, "No help found for '" + query + "'. Try ", $format.annotation:command("help", "help"), " to see available topics."):with_audience('utility));
    endif
  endverb

  method _show_location_help owner: ARCH_WIZARD
    "Show help for current location context. Returns modified lines list.";
    context_block = `$help_utils:display_location_context(this) ! ANY => 0';
    lines = {};
    if (typeof(context_block) == TYPE_FLYWEIGHT)
      lines = {@lines, context_block};
    endif
    cmd_env = this:command_environment();
    ambient_verbs = $obj_utils:collect_ambient_verbs(cmd_env);
    lines = this:_display_ambient_verbs(ambient_verbs, lines);
    if (length(ambient_verbs) == 0)
      lines = {@lines, "(No commands available)"};
    endif
    lines = {@lines, $format.title:mk("Need more detail?", 4)};
    lines = {@lines, $format.code:mk("help <thing>\nexamine <thing>")};
    lines = {@lines, "Use these on anything listed above to see its specific commands or description."};
    "Show developer documentation hint for programmers";
    if (this.programmer)
      lines = {@lines, $format.title:mk("To look for programmer documentation...")};
      lines = {@lines, $format.code:mk("@doc object\n@doc object:verb")};
    endif
    return lines;
  endmethod

  method _show_targeted_help owner: ARCH_WIZARD
    "Show help for a targeted object. Returns {lines_list, has_documentation_flag}.";
    {target_obj} = args;
    has_doc = false;
    cmd_env = this:command_environment();
    lines = {};
    "If target is in command environment, show ambient verbs";
    if (cmd_env && target_obj in cmd_env)
      ambient_verbs = $obj_utils:collect_ambient_verbs(cmd_env);
      if (ambient_verbs && length(ambient_verbs) > 0)
        lines = {@lines, ""};
        lines = {@lines, $format.title:mk("Things you can do", 4)};
        verbs = {};
        for verb_info in (ambient_verbs)
          verbs = {@verbs, verb_info["annotation"]};
        endfor
        lines = {@lines, $format.list:mk(verbs, false)};
      else
        lines = {@lines, "(No commands available)"};
      endif
      return {lines, has_doc};
    endif
    "Show help for target object";
    try
      help_content = target_obj:object_help();
    except e (ANY)
      raise(e[1], "Object help provider failed on " + tostr(target_obj) + ": " + toliteral(e));
    endtry
    if (help_content && help_content != 0)
      has_doc = true;
      if (typeof(help_content) == TYPE_LIST)
        lines = {@lines, @help_content};
      else
        lines = {@lines, help_content};
      endif
    endif
    "Get targetable verbs for this object";
    targetable_verbs = $obj_utils:collect_targetable_verbs({target_obj});
    if (targetable_verbs && length(targetable_verbs) > 0)
      lines = {@lines, ""};
      for obj_info in (targetable_verbs)
        lines = {@lines, $format.title:mk("Things you can do with " + obj_info["object_name"], 4)};
        lines = {@lines, $format.list:mk(obj_info["verbs"], false)};
      endfor
    else
      lines = {@lines, "(No commands available for this object)"};
    endif
    return {lines, has_doc};
  endmethod

  method _display_targetable_verbs owner: ARCH_WIZARD
    "Display targetable verbs grouped by object. Returns modified lines list.";
    {targetable_verbs, lines} = args;
    if (!(targetable_verbs && length(targetable_verbs) > 0))
      return lines;
    endif
    lines = {@lines, ""};
    for obj_info in (targetable_verbs)
      lines = {@lines, $format.title:mk("Things you can do with " + obj_info["object_name"], 4)};
      lines = {@lines, $format.list:mk(obj_info["verbs"], false)};
    endfor
    return lines;
  endmethod

  method _display_ambient_verbs owner: ARCH_WIZARD
    "Display ambient verbs, separated into player and room verbs. Returns modified lines list.";
    {ambient_verbs, lines} = args;
    if (!(ambient_verbs && length(ambient_verbs) > 0))
      return lines;
    endif
    player_verbs = {};
    room_verbs = {};
    location = this.location;
    "Separate verbs by source object";
    for verb_info in (ambient_verbs)
      if (verb_info["from_object"] == location)
        room_verbs = {@room_verbs, verb_info["annotation"]};
      else
        player_verbs = {@player_verbs, verb_info["annotation"]};
      endif
    endfor
    "Display player commands (including features)";
    if (player_verbs && length(player_verbs) > 0)
      lines = {@lines, ""};
      lines = {@lines, $format.title:mk("Things you can do", 4)};
      lines = {@lines, $format.list:mk(player_verbs, false)};
    endif
    "Display room commands";
    if (room_verbs && length(room_verbs) > 0)
      lines = {@lines, ""};
      lines = {@lines, $format.title:mk("Things you can do in this room", 4)};
      lines = {@lines, $format.list:mk(room_verbs, false)};
    endif
    return lines;
  endmethod

  method help_environment owner: ARCH_WIZARD
    "Return list of objects to search for help topics.";
    "Order: global, player, features (including authoring/admin/builder/wizard), room, inventory, room contents";
    const principal = caller_perms();
    principal == this || (valid(principal) && principal.wizard) || raise(E_PERM);
    set_task_perms(this);
    env = {};
    gh = `$sysobj.help_topics ! E_PROPNF => $nothing';
    if (valid(gh))
      env = {@env, gh};
    endif
    env = {@env, this, @this:_feature_environment()};
    if (valid(this.location))
      env = {@env, this.location};
    endif
    for item in (this.contents)
      env = {@env, item};
    endfor
    if (valid(this.location))
      for item in (this.location.contents)
        if (item != this)
          env = {@env, item};
        endif
      endfor
    endif
    return env;
  endmethod

  method _server_log owner: ARCH_WIZARD
    "Write a server log entry without keeping the caller's full wizard authority.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    set_task_perms(this, {{"builtin_call", "server_log"}});
    server_log(@args);
  endmethod

  method find_help_topic owner: ARCH_WIZARD
    "Search environment for help topics. Returns single $help flyweight, list of {source, topic} for ambiguous, or 0.";
    {topic} = args;
    const principal = caller_perms();
    principal == this || (valid(principal) && principal.wizard) || raise(E_PERM);
    set_task_perms(this);
    env = this:help_environment();
    matches = {};
    seen_sources = [];
    for o in (env)
      if (typeof(o) != TYPE_OBJ || !valid(o))
        continue;
      endif
      if (maphaskey(seen_sources, o))
        continue;
      endif
      seen_sources[o] = true;
      if (!o.r)
        continue;
      endif
      if (!respond_to(o, 'help_topics))
        continue;
      endif
      try
        result = o:help_topics(this, topic);
      except e (ANY)
        raise(e[1], "Help topic provider failed on " + tostr(o) + ": " + toliteral(e));
      endtry
      if (typeof(result) != TYPE_INT)
        matches = {@matches, {o, result}};
      endif
    endfor
    length(matches) == 0 && return 0;
    length(matches) == 1 && return matches[1][2];
    return matches;
  endmethod

  method _collect_help_topics owner: ARCH_WIZARD
    "Gather all help topics from the environment.";
    "Returns a deduplicated list of help topic flyweights by topic name.";
    const principal = caller_perms();
    principal == this || (valid(principal) && principal.wizard) || raise(E_PERM);
    set_task_perms(this);
    env = this:help_environment();
    all_topics = {};
    seen_sources = [];
    seen_topic_names = [];
    for o in (env)
      if (typeof(o) != TYPE_OBJ || !valid(o))
        continue;
      endif
      if (maphaskey(seen_sources, o))
        continue;
      endif
      seen_sources[o] = true;
      if (!o.r)
        continue;
      endif
      if (!respond_to(o, 'help_topics))
        continue;
      endif
      try
        topics = o:help_topics(this, "");
      except e (ANY)
        this:_server_log("[help] topic collection failed for " + tostr(o) + ": " + toliteral(e));
        continue;
      endtry
      if (typeof(topics) != TYPE_LIST)
        continue;
      endif
      for t in (topics)
        if (typeof(t) != TYPE_FLYWEIGHT)
          continue;
        endif
        name = `t.name ! ANY => ""';
        if (typeof(name) != TYPE_STR || name == "")
          continue;
        endif
        key = name:lowercase();
        if (maphaskey(seen_topic_names, key))
          continue;
        endif
        seen_topic_names[key] = true;
        all_topics = {@all_topics, t:from_provider(o)};
      endfor
    endfor
    return all_topics;
  endmethod

  method available_help owner: ARCH_WIZARD
    "Return structured list of all help available in current context.";
    "For use by LLM agents and programmatic discovery.";
    topics = this:_collect_help_topics();
    result = {};
    seen = {};
    for t in (topics)
      if (!(t.name in seen))
        seen = {@seen, t.name};
        result = {@result, t:render_structured()};
      endif
    endfor
    return result;
  endmethod

  method present_editor owner: ARCH_WIZARD
    "Open a text editor panel for the player.";
    "Args: target_obj, verb_name, ?initial_content, ?opts";
    "opts keys: content_type ('text_plain or 'text_djot), title, text_mode ('list or 'string), session_id";
    "On save calls: target_obj:verb_name(content)";
    const actor = caller_perms();
    actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    const {target_obj, verb_name, ?initial_content = "", ?opts = []} = args;
    const content_type = `opts['content_type] ! E_RANGE => 'text_plain';
    const title = `opts['title] ! E_RANGE => "Edit"';
    const text_mode = `opts['text_mode] ! E_RANGE => 'string';
    const session_id = `opts['session_id] ! E_RANGE => ""';
    const ct_str = content_type == 'text_djot ? "text/djot" | "text/plain";
    const mode_str = text_mode == 'string ? "string" | "list";
    const attrs = {{"object", $url_utils:to_curie_str(target_obj)}, {"verb", verb_name}, {"title", title}, {"text_mode", mode_str}};
    present(this, session_id, ct_str, "text-editor", initial_content, attrs);
  endmethod

  method start_edit_session owner: ARCH_WIZARD
    "Create an editing session for tracking editor callbacks.";
    "Args: target_obj, verb_name, ?extra_args";
    "Returns: session_id string";
    const actor = caller_perms();
    actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    const {target_obj, verb_name, ?extra_args = {}} = args;
    const session_id = uuid();
    this.editing_sessions[session_id] = ['target -> target_obj, 'verb -> verb_name, 'args -> extra_args];
    return session_id;
  endmethod

  method get_edit_session owner: ARCH_WIZARD
    "Get an editing session by session_id. Returns session data or E_INVARG if not found.";
    const actor = caller_perms();
    actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    const {session_id} = args;
    return `this.editing_sessions[session_id] ! E_RANGE => raise(E_INVARG, "No such editing session")';
  endmethod

  method end_edit_session owner: ARCH_WIZARD
    "End an editing session, removing it from the map. Returns session data.";
    const actor = caller_perms();
    actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    const {session_id} = args;
    const session = `this.editing_sessions[session_id] ! E_RANGE => raise(E_INVARG, "No such editing session")';
    this.editing_sessions = mapdelete(this.editing_sessions, session_id);
    return session;
  endmethod

  method confirm_with_all owner: ARCH_WIZARD
    "Show a confirmation prompt with Yes/Yes to All/No/Alternative options.";
    "Returns: true (yes), 'yes_all' (accept all), false (no), or string (alternative)";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    {message, ?alt_label = "Or suggest an alternative:", ?alt_placeholder = "Describe your alternative approach...", ?tts_prompt = 0} = args;
    metadata = {{"input_type", "yes_no_alternative_all"}, {"prompt", message}, {"alternative_label", alt_label}, {"alternative_placeholder", alt_placeholder}};
    typeof(tts_prompt) == TYPE_STR && (metadata = {@metadata, {"tts_prompt", tts_prompt}});
    response = this:read_with_prompt(metadata);
    set_task_perms(this);
    if (response == "yes")
      this:inform_current($event:mk_info(this, "Confirmed."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return true;
    elseif (response == "yes_all")
      this:inform_current($event:mk_info(this, "Confirmed (all future changes will be auto-accepted)."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return 'yes_all;
    elseif (response == "no")
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    elseif (index(response, "alternative: ") == 1)
      alt_text = response[14..$];
      this:inform_current($event:mk_info(this, "Alternative provided: " + alt_text):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return alt_text;
    else
      "Fallback for unexpected responses";
      this:inform_current($event:mk_info(this, "Cancelled."):with_audience('utility):with_presentation_hint('inset):with_group('utility, this));
      return false;
    endif
  endmethod

  method command_input_context owner: ARCH_WIZARD
    "Describe the object argument at a composer cursor using the ordinary parser. Query only; never dispatch.";
    const {before_cursor, after_cursor} = args;
    valid(player) && is_player(player) && this == player || raise(E_PERM);
    typeof(before_cursor) == TYPE_STR && typeof(after_cursor) == TYPE_STR || raise(E_TYPE);
    const command = before_cursor + after_cursor;
    length(command) <= 1024 && !index(command, "\n") && !index(command, "\r") || return [];
    set_task_perms(player);
    const marker = "cursor" + uuid();
    const probe = before_cursor + marker + after_cursor;
    const parsed = parse_command(probe, {}, false);
    const slot = index(parsed['dobjstr], marker) ? "dobj" | index(parsed['iobjstr], marker) ? "iobj" | "";
    !length(slot) && return [];
    const argument = parsed[slot == "dobj" ? 'dobj_raw | 'iobj_raw];
    const span = parsed[slot == "dobj" ? 'dobj_span | 'iobj_span];
    const arg_start = length(probe) - length(parsed['argstr]);
    const before = probe[1..arg_start + span[1] - 1];
    const after = probe[arg_start + span[2] + 1..$];
    const query = strsub(argument, marker, "");
    before + query + after == command || return [];
    const template = before + "{input}" + after;
    const context = $match:input_context(template);
    return ["before" -> before, "after" -> after, "query" -> query, "label" -> slot == "dobj" ? "direct object" | "indirect object", "source" -> ["provider" -> $url_utils:to_curie_str(player), "source" -> "nearby", "context" -> context]];
  endmethod

  method suggestion_candidates owner: ARCH_WIZARD
    "Add command-name completion to the shared environment-based object sources.";
    const {source, ?query = "", ?context = []} = args;
    this == player && (caller_perms() == this || caller_perms().wizard) || raise(E_PERM);
    set_task_perms(this);
    source != "commands" && return pass(@args);
    let result = {};
    for entry in (this:verb_suggestions())
      const names = { strsub(alias, "*", "") for alias in (entry['verb]:words()) };
      const name = names[1];
      result = {@result, ["id" -> name, "label" -> name, "value" -> name, "detail" -> entry['hint], "keys" -> names]};
    endfor
    return result;
  endmethod

  method verb_suggestions owner: ARCH_WIZARD
    "Return list of verb info for UI suggestion pills.";
    "Each entry: [verb -> name, dobj -> spec, prep -> spec, iobj -> spec, objects -> {objs}, hint -> prompt hint]";
    "Priority verbs appear first, then ambient verbs from the environment.";
    caller == this || caller_perms() == this || caller_perms().wizard || raise(E_PERM);
    priority_patterns = {"say", "emote", "go", "look", "l*ook", "examine", "exam*ine", "inventory", "i*nventory", "help", "what", "who"};
    result = {};
    seen = [];
    "Collect all ambient verbs from command environment";
    cmd_env = this:command_environment();
    all_verbs = [];
    for o in (cmd_env)
      if (!valid(o))
        continue;
      endif
      ancestor_chain = ancestors(o);
      for definer in ({o, @ancestor_chain})
        if (!valid(definer))
          continue;
        endif
        all_verb_names = verbs(definer);
        for verb_name in (all_verb_names)
          verb_sig = verb_args(definer, verb_name);
          if (typeof(verb_sig) != TYPE_LIST || length(verb_sig) < 3)
            continue;
          endif
          {dobj, prep, iobj} = verb_sig;
          "Skip targetable verbs (those requiring 'this' as dobj/iobj)";
          if (dobj == "this" || iobj == "this")
            continue;
          endif
          if (maphaskey(all_verbs, verb_name))
            "Add this object to the list of applicable objects";
            entry = all_verbs[verb_name];
            objs = entry['objects];
            if (!(o in objs))
              entry['objects] = {@objs, o};
              all_verbs[verb_name] = entry;
            endif
          else
            hint = this:_make_verb_hint(definer, verb_name, dobj, prep, iobj);
            all_verbs[verb_name] = ['verb -> verb_name, 'dobj -> dobj, 'prep -> prep, 'iobj -> iobj, 'objects -> {o}, 'hint -> hint, 'action -> definer:command_action(verb_name)];
          endif
        endfor
      endfor
    endfor
    "Build result with priority verbs first";
    for pv in (priority_patterns)
      if (maphaskey(all_verbs, pv))
        result = {@result, all_verbs[pv]};
        seen[pv] = true;
      endif
    endfor
    "Add remaining verbs";
    for verb_name in (mapkeys(all_verbs))
      if (!maphaskey(seen, verb_name))
        result = {@result, all_verbs[verb_name]};
      endif
    endfor
    "Add placeholder text to first entry for input field hint";
    if (length(result) > 0)
      result[1]['placeholder_text] = this:input_placeholder();
    endif
    return result;
  endmethod

  method _make_verb_hint owner: ARCH_WIZARD
    "Generate a prompt hint for a verb.";
    "First checks for HINT: tag in verb comment, falls back to argspec-based hint.";
    "HINT format: 'HINT: <whom> -- Description' returns '<whom> -- Description'";
    {definer, verb_name, dobj, prep, iobj} = args;
    "Try to get hint from verb comment";
    code = `verb_code(definer, verb_name) ! E_VERBNF => {}';
    if (code && length(code) > 0)
      first_line = code[1]:trim();
      "Check if it's a string literal (comment) starting with HINT:";
      if (first_line:starts_with("\"HINT:"))
        "Extract content between quotes";
        end_quote = rindex(first_line, "\"");
        if (end_quote > 6)
          return first_line[7..end_quote - 1]:trim();
        endif
      endif
    endif
    "Fall back to argspec-based hint";
    if (dobj == "none" && iobj == "none")
      return "";
    endif
    parts = {};
    "Add dobj placeholder";
    if (dobj == "any")
      parts = {"<what>"};
    endif
    "Add preposition if meaningful";
    if (prep != "none" && prep != "any" && iobj == "any")
      "Clean up prep - some have slashes like 'in/inside/into'";
      slash = index(prep, "/");
      display_prep = slash > 0 ? prep[1..slash - 1] | prep;
      parts = {@parts, display_prep, "<whom>"};
    elseif (iobj == "any" && prep == "any")
      parts = {@parts, "..."};
    endif
    return parts:join(" ");
  endmethod

  verb "dm pm tell page" (any any any) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["label" -> "Send message", "prefix" -> "Message"]
  ]
    "Send a direct message to another player.";
    "Usage: dm <player> <message>";
    caller == this || raise(E_PERM);
    const actor = caller_perms();
    actor == #-1 || actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(this);
    const {?target_name = "", @words} = args;
    if (length(words) == 0)
      return this:inform_current($event:mk_error(this, "Usage: " + verb + " <player> <message>"):with_audience('utility));
    endif
    "First arg is player name, rest is message";
    const message = words:join(" ");
    if (!target_name || !message)
      return this:inform_current($event:mk_error(this, "Usage: " + verb + " <player> <message>"):with_audience('utility));
    endif
    "Match target player";
    const target = `$match:match_player(target_name) ! E_INVARG => $failed_match';
    if (target == $failed_match || !valid(target) || !is_player(target))
      return this:inform_current($event:mk_error(this, "I don't know who '" + target_name + "' is."):with_audience('utility));
    endif
    if (target == this)
      return this:inform_current($event:mk_error(this, "Talking to yourself?"):with_audience('utility));
    endif
    "Create and deliver the DM";
    const dm_obj = $dm:mk(this, target, message);
    let delivered = false;
    try
      delivered = target:receive_dm(dm_obj);
    except e (ANY)
      return this:inform_current($event:mk_error(this, "Couldn't deliver your message to " + target:name() + ": " + toliteral(e[2])):with_audience('utility));
    endtry
    if (typeof(delivered) == TYPE_ERR || !delivered)
      return this:inform_current($event:mk_error(this, "Couldn't deliver your message to " + target:name() + "."):with_audience('utility));
    endif
    "Echo to sender only on successful delivery";
    this:inform_current(dm_obj:sender_echo_event());
  endverb

  method receive_dm owner: ARCH_WIZARD
    "Accept a validated DM from its authenticated sender before notifying live sessions.";
    const {dm_obj} = args;
    const actor = caller_perms();
    typeof(dm_obj) == TYPE_FLYWEIGHT && dm_obj.delegate == $dm || raise(E_TYPE);
    const slots = flyslots(dm_obj);
    const sender = slots['from];
    const recipient = slots['to];
    const text = slots['text];
    const sent = slots['sent];
    const location = slots['location];
    typeof(sender) == TYPE_OBJ && valid(sender) || raise(E_INVARG);
    actor == sender || actor.wizard || raise(E_PERM);
    recipient == this || raise(E_INVARG);
    typeof(text) == TYPE_STR && typeof(sent) == TYPE_INT && typeof(location) == TYPE_OBJ || raise(E_TYPE);
    const accepted = toflyweight($dm, ['from -> sender, 'to -> this, 'text -> text, 'sent -> sent, 'location -> location]);
    const event = accepted:display_event(this);
    let buffer = {@this.direct_messages, accepted};
    if (length(buffer) > 100)
      buffer = buffer[length(buffer) - 99..$];
    endif
    this.direct_messages = buffer;
    this.last_dm_from = sender;
    "Storage is delivery; a failed live notification must not report the stored DM as rejected.";
    try
      this:tell(event);
    except error (ANY)
      server_log("DM notification failed for " + tostr(this) + ": " + tostr(error[1]));
    endtry
    return true;
  endmethod

  verb reply (any any any) owner: ARCH_WIZARD flags: "rd" [ ui -> ["prefix" -> "Reply to"] ]
    "Reply to the last person who DM'd you.";
    "Usage: reply <message>";
    caller == this || raise(E_PERM);
    const actor = caller_perms();
    actor == #-1 || actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(this);
    const target = this.last_dm_from;
    if (!valid(target) || !is_player(target))
      this.last_dm_from = #-1;
      return this:inform_current($event:mk_error(this, "No one to reply to."):with_audience('utility));
    endif
    if (!args || length(args) < 1)
      return this:inform_current($event:mk_error(this, "Usage: reply <message>"):with_audience('utility));
    endif
    const message = args:join(" ");
    "Create and deliver the DM";
    const dm_obj = $dm:mk(this, target, message);
    let delivered = false;
    try
      delivered = target:receive_dm(dm_obj);
    except e (ANY)
      return this:inform_current($event:mk_error(this, "Couldn't deliver your message to " + target:name() + ": " + toliteral(e[2])):with_audience('utility));
    endtry
    if (typeof(delivered) == TYPE_ERR || !delivered)
      return this:inform_current($event:mk_error(this, "Couldn't deliver your message to " + target:name() + "."):with_audience('utility));
    endif
    "Echo to sender only on successful delivery";
    this:inform_current(dm_obj:sender_echo_event());
  endverb

  verb "dms messages msgs mail" (none none none) owner: ARCH_WIZARD flags: "rd" [
    ui -> ["label" -> "Messages"]
  ]
    "Show all messages (DMs and mail) in unified view.";
    caller == this || raise(E_PERM);
    const actor = caller_perms();
    actor == #-1 || actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(this);
    "Ensure mailbox exists";
    let mailbox = this:find_mailbox();
    if (!valid(mailbox))
      mailbox = create($mailbox, this);
      mailbox.name = this.name + "'s mailbox";
      move(mailbox, $mail_room);
    endif
    const messages = this:all_messages();
    if (!messages || length(messages) == 0)
      return this:inform_current($event:mk_info(this, "No messages."):with_audience('utility):with_group('messages));
    endif
    "Build table";
    const headers = {"#", "Type", "From", "Subject", "When"};
    let rows = {};
    let idx = 1;
    for msg in (messages)
      let msg_type = "";
      let from_name = "";
      let preview = "";
      let age = 0;
      let when = "";
      if (typeof(msg) == TYPE_FLYWEIGHT)
        "DM flyweight - show full text";
        msg_type = "DM";
        from_name = valid(msg.from) ? msg.from.name | "???";
        preview = msg.text;
        age = time() - msg.sent;
        when = age < 60 ? "now" | age:format_time_seconds();
      else
        "Letter object";
        msg_type = msg.read_at == 0 ? "letter*" | "letter";
        from_name = valid(msg.author) ? msg.author.name | "???";
        preview = msg.subject ? msg.subject | "(no subject)";
        age = time() - msg.sent_at;
        when = age < 60 ? "now" | age:format_time_seconds();
      endif
      rows = {@rows, {tostr(idx), msg_type, from_name, preview, when}};
      idx = idx + 1;
    endfor
    "Count unread";
    let unread = 0;
    for msg in (messages)
      if (typeof(msg) != TYPE_FLYWEIGHT && msg.read_at == 0)
        unread = unread + 1;
      endif
    endfor
    let summary = tostr(length(messages), " message", length(messages) == 1 ? "" | "s");
    if (unread > 0)
      summary = summary + tostr(" (", unread, " unread)");
    endif
    const title = $format.title:mk("Messages: " + summary);
    const parts = {title, $format.table:mk(headers, rows), "", "To read: message <#>"};
    const content = $format.block:mk(@parts);
    this:inform_current($event:mk_info(this, content):with_audience('utility):with_presentation_hint('inset):with_group('messages));
  endverb

  method all_messages owner: ARCH_WIZARD
    "Return unified list of all messages (DMs and letters), sorted by time.";
    const actor = caller_perms();
    actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    let msgs = {};
    "Add DMs with their timestamps";
    for dm_obj in (this.direct_messages)
      const t = dm_obj.sent;
      msgs = {@msgs, {t, dm_obj}};
    endfor
    "Add letters from mailbox";
    const mailbox = this:find_mailbox();
    if (valid(mailbox))
      for letter in (mailbox.contents)
        if (isa(letter, $letter))
          const t = letter.sent_at;
          msgs = {@msgs, {t, letter}};
        endif
      endfor
    endif
    "Sort by timestamp (newest first) using simple insertion sort";
    let sorted = {};
    for item in (msgs)
      const {t, msg} = item;
      let inserted = false;
      for i in [1..length(sorted)]
        if (t > sorted[i][1])
          sorted = {@sorted[1..i - 1], item, @sorted[i..$]};
          inserted = true;
          break;
        endif
      endfor
      if (!inserted)
        sorted = {@sorted, item};
      endif
    endfor
    "Extract just the messages";
    return { m[2] for m in (sorted) };
  endmethod

  verb message (any none none) owner: ARCH_WIZARD flags: "rd"
    "Read a specific message by number.";
    "Usage: message <#>";
    caller == this || raise(E_PERM);
    const actor = caller_perms();
    actor == #-1 || actor == this || (valid(actor) && actor.wizard) || raise(E_PERM);
    set_task_perms(this);
    if (!dobjstr || !dobjstr:is_numeric())
      return this:inform_current($event:mk_error(this, "Usage: message <#>"):with_audience('utility));
    endif
    let idx = toint(dobjstr);
    const messages = this:all_messages();
    if (idx < 1 || idx > length(messages))
      return this:inform_current($event:mk_error(this, "No message #" + tostr(idx) + "."):with_audience('utility));
    endif
    const msg = messages[idx];
    if (typeof(msg) == TYPE_FLYWEIGHT)
      "DM - display it";
      const display = msg:display(this);
      this:inform_current($event:mk_info(this, display):with_audience('utility):with_presentation_hint('inset):with_group('messages));
    else
      "Letter";
      msg:action_read(this, []);
    endif
  endverb

  verb suggest_command_alternatives (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Offer bounded LLM command assistance on the invoking connection without starting model work.";
    $llm_tool_provider:_require_tool_dispatch(this, caller_perms());
    const {input} = args;
    const destination = `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return false;
    set_task_perms(this);
    const token = $llm_assist_context:offer(this, destination, 'command, input);
    length(token) > 0 || return false;
    const message = $format.paragraph:mk("I didn't understand that command. Want help checking alternatives? ",
      $format.annotation:command("assist --request " + token, "Yes"));
    this:inform_connection(destination, $event:mk_do_not_understand(this, message):with_audience('utility));
    return true;
  endverb

  verb suggest_help_topic (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Offer bounded LLM help assistance on the invoking connection without starting model work.";
    $llm_tool_provider:_require_tool_dispatch(this, caller_perms());
    const {input} = args;
    const destination = `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return false;
    set_task_perms(this);
    const token = $llm_assist_context:offer(this, destination, 'help, input);
    length(token) > 0 || return false;
    const message = $format.paragraph:mk("No help found for '" + input + "'. Want help finding similar topics? ",
      $format.annotation:command("assist --request " + token, "Yes"));
    this:inform_connection(destination, $event:mk_error(this, message):with_audience('utility));
    return true;
  endverb

  method set_home owner: ARCH_WIZARD
    "Set this player's home room. Permission: wizard, owner, or 'set_home capability.";
    const actor = caller_perms();
    const {target, perms} = $root:_check_permissions_as(this, actor, 'set_home);
    set_task_perms(perms);
    const {room} = args;
    target.home = room;
  endmethod

  verb "@gag" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Add a player or object to your gag list.";
    "Usage: @gag <player|object>";
    caller != this && raise(E_PERM);
    set_task_perms(this);
    if (!args || length(args) < 1)
      usage = $format.block:mk($format.title:mk("Usage"), $format.code:mk("@gag <player|object>"));
      event = $event:mk_error(this, usage):with_audience('utility):as_inset():with_group('utility, this);
      return this:inform_current(event);
    endif
    ref = args:join(" ");
    target = 0;
    "If it looks like an explicit object reference (#..., $..., @...), resolve it as such.";
    if (length(ref) >= 1 && ref[1] in {"#", "$", "@"})
      try
        target = $match:match_object(ref, this);
      except e (E_INVARG, E_PROPNF)
        return this:inform_current($event:mk_error(this, "I can't find '" + ref + "' to gag."):with_audience('utility));
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Object lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
    else
      "Try matching a player anywhere by name.";
      try
        target = $match:match_player(ref, this);
      except e (E_INVARG)
        "Fall back to matching an object in scope (here, inventory, room, etc.).";
        try
          target = $match:match_object(ref, this);
        except e2 (E_INVARG, E_PROPNF)
          return this:inform_current($event:mk_error(this, "I can't find '" + ref + "' to gag."):with_audience('utility));
        except e2 (ANY)
          return this:inform_current($event:mk_error(this, "Object lookup failed: " + toliteral(e2)):with_audience('utility));
        endtry
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Player lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
    endif
    if (target == this)
      return this:inform_current($event:mk_error(this, "You can't gag yourself."):with_audience('utility));
    endif
    if (is_player(target))
      gl = this.gaglist;
      if (target in gl)
        return this:inform_current($event:mk_error(this, "Already gagged: " + target:noun_phrase('definite, this) + "."):with_audience('utility));
      endif
      this.gaglist = {@gl, target};
      this:inform_current($event:mk_say(this, "You gag ", target:noun_phrase('definite, this), "."):with_audience('utility));
    else
      ogl = this.object_gaglist;
      if (target in ogl)
        return this:inform_current($event:mk_error(this, "Already gagged: " + target:noun_phrase('definite, this) + "."):with_audience('utility));
      endif
      this.object_gaglist = {@ogl, target};
      this:inform_current($event:mk_say(this, "You gag ", target:noun_phrase('definite, this), "."):with_audience('utility));
    endif
    return this:listgag();
  endverb

  verb "@ungag" (any any any) owner: ARCH_WIZARD flags: "rxd"
    "Remove a player or object from your gag list.";
    "Usage: @ungag <player|object>";
    "       @ungag everyone";
    caller != this && raise(E_PERM);
    set_task_perms(this);
    if (!args || length(args) < 1)
      usage = $format.block:mk($format.title:mk("Usage"), $format.code:mk("@ungag <player|object>\n@ungag everyone"));
      event = $event:mk_error(this, usage):with_audience('utility):as_inset():with_group('utility, this);
      return this:inform_current(event);
    endif
    ref = args:join(" ");
    if (ref == "everyone")
      this.gaglist = {};
      this.object_gaglist = {};
      this:inform_current($event:mk_say(this, "You clear your gag lists."):with_audience('utility));
      return this:listgag();
    endif
    "Try to resolve a player/object reference first.";
    target = 0;
    if (length(ref) >= 1 && ref[1] in {"#", "$", "@"})
      try
        target = $match:match_object(ref, this);
      except e (E_INVARG, E_PROPNF)
        target = 0;
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Object lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
    else
      try
        target = $match:match_player(ref, this);
      except e (E_INVARG)
        try
          target = $match:match_object(ref, this);
        except e2 (E_INVARG, E_PROPNF)
          target = 0;
        except e2 (ANY)
          return this:inform_current($event:mk_error(this, "Object lookup failed: " + toliteral(e2)):with_audience('utility));
        endtry
      except e (ANY)
        return this:inform_current($event:mk_error(this, "Player lookup failed: " + toliteral(e)):with_audience('utility));
      endtry
    endif
    if (typeof(target) == TYPE_OBJ && valid(target))
      if (is_player(target))
        gl = this.gaglist;
        if (!(target in gl))
          return this:inform_current($event:mk_error(this, "Not gagged: " + target:noun_phrase('definite, this) + "."):with_audience('utility));
        endif
        new = {};
        for o in (gl)
          if (o != target)
            new = {@new, o};
          endif
        endfor
        this.gaglist = new;
        this:inform_current($event:mk_say(this, "You ungag ", target:noun_phrase('definite, this), "."):with_audience('utility));
        return this:listgag();
      else
        ogl = this.object_gaglist;
        if (!(target in ogl))
          return this:inform_current($event:mk_error(this, "Not gagged: " + target:noun_phrase('definite, this) + "."):with_audience('utility));
        endif
        new = {};
        for o in (ogl)
          if (o != target)
            new = {@new, o};
          endif
        endfor
        this.object_gaglist = new;
        this:inform_current($event:mk_say(this, "You ungag ", target:noun_phrase('definite, this), "."):with_audience('utility));
        return this:listgag();
      endif
    endif
    "If it didn't resolve, attempt to match against existing gag lists.";
    match = 0;
    try
      match = $match:resolve_in_scope(ref, this.gaglist);
    except e (ANY)
      match = $failed_match;
    endtry
    if (match == $ambiguous_match)
      return this:inform_current($event:mk_error(this, "More than one gagged object matches. Use a more specific name."));
    endif
    if (match != $failed_match)
      gl = this.gaglist;
      new = {};
      for o in (gl)
        if (o != match)
          new = {@new, o};
        endif
      endfor
      this.gaglist = new;
      this:inform_current($event:mk_say(this, "You ungag ", match:noun_phrase('definite, this), "."):with_audience('utility));
      return this:listgag();
    endif
    try
      match = $match:resolve_in_scope(ref, this.object_gaglist);
    except e (ANY)
      match = $failed_match;
    endtry
    if (match == $ambiguous_match)
      return this:inform_current($event:mk_error(this, "More than one gagged object matches. Use a more specific name."));
    endif
    if (match != $failed_match)
      ogl = this.object_gaglist;
      new = {};
      for o in (ogl)
        if (o != match)
          new = {@new, o};
        endif
      endfor
      this.object_gaglist = new;
      this:inform_current($event:mk_say(this, "You ungag ", match:noun_phrase('definite, this), "."):with_audience('utility));
      return this:listgag();
    endif
    return this:inform_current($event:mk_error(this, "'" + ref + "' isn't on your gag lists."):with_audience('utility));
  endverb

  verb "@listgag listgag" (none none none) owner: ARCH_WIZARD flags: "rxd"
    "List players/objects you have gagged, and (optionally) who has gagged you.";
    caller != this && raise(E_PERM);
    set_task_perms(this);
    from_command = !callers();
    parts = {$format.title:mk("Gag Lists")};
    "Player gag list";
    player_names = {};
    if (this.gaglist && length(this.gaglist) > 0)
      for p in (this.gaglist)
        player_names = {@player_names, this:_format_gag_entry(p)};
      endfor
    endif
    parts = {@parts, "Gagged players:"};
    if (length(player_names) > 0)
      parts = {@parts, $format.list:mk(player_names)};
    else
      parts = {@parts, "(none)"};
    endif
    "Object gag list";
    object_names = {};
    if (this.object_gaglist && length(this.object_gaglist) > 0)
      for o in (this.object_gaglist)
        object_names = {@object_names, this:_format_gag_entry(o)};
      endfor
    endif
    parts = {@parts, "", "Gagged objects:"};
    if (length(object_names) > 0)
      parts = {@parts, $format.list:mk(object_names)};
    else
      parts = {@parts, "(none)"};
    endif
    "Only do the database scan when invoked as a player command.";
    if (from_command)
      gagger_names = {};
      for p in (players())
        if (p != this)
          try
            if (this in p.gaglist)
              gagger_names = {@gagger_names, p:name()};
            endif
          except e (E_PROPNF)
          endtry
        endif
      endfor
      if (length(gagger_names) > 0)
        parts = {@parts, "", "You are gagged by:", $format.list:mk(gagger_names)};
      endif
    endif
    parts = {@parts, "", "Usage:", $format.code:mk("@gag <player|object>\n@ungag <player|object>\n@ungag everyone")};
    content = $format.block:mk(@parts);
    event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
    return this:inform_current(event);
  endverb

  method _format_gag_entry owner: ARCH_WIZARD
    "Format one gag-list entry for display, including malformed entries.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {entry} = args;
    if (typeof(entry) != TYPE_OBJ || !valid(entry))
      return "(invalid entry: " + toliteral(entry) + ")";
    endif
    try
      return entry:name();
    except e (ANY)
      return "(error reading " + tostr(entry) + ": " + toliteral(e) + ")";
    endtry
  endmethod

  method _display_ambiguous_topic_matches owner: ARCH_WIZARD
    "Display a nicely formatted ambiguous-topic help panel.";
    {query, matches} = args;
    typeof(query) != TYPE_STR && (query = tostr(query));
    typeof(matches) != TYPE_LIST && (matches = {});
    items = {};
    for match in (matches)
      if (typeof(match) != TYPE_LIST || length(match) < 2)
        continue;
      endif
      {source, topic} = match;
      if (typeof(topic) != TYPE_FLYWEIGHT)
        continue;
      endif
      source_name = `source.name ! ANY => tostr(source)';
      summary = `topic.summary ! ANY => ""';
      cat = `tostr(topic.category) ! ANY => ""';
      "Primary (novice-friendly) disambiguation: natural language.";
      lines = {};
      header = topic.name;
      summary && (header = header + " - " + summary);
      header = header + " (from " + source_name + ")";
      lines = {@lines, header};
      lines = {@lines, $format.paragraph:mk("Try: ", $format.code:inline("help " + topic.name + " from " + source_name))};
      "Secondary (expert) options.";
      if (this.programmer)
        source_ref = toliteral(source);
        source == $prog_features && (source_ref = "$prog_features");
        source == $sysobj.help_topics && (source_ref = "$sysobj.help_topics");
        lines = {@lines, $format.paragraph:mk("Advanced: ", $format.code:inline("help " + source_ref + ":" + topic.name))};
        cat && (lines = {@lines, $format.paragraph:mk("Advanced: ", $format.code:inline("help " + cat + " " + topic.name))});
      endif
      items = {@items, $format.block:mk(@lines)};
    endfor
    content = $format.block:mk($format.title:mk("Ambiguous Topic"), $format.paragraph:mk("Multiple help topics match '" + query + "':"), $format.list:mk(items), $format.paragraph:mk("Be more specific, or browse with: ", $format.code:inline("help source")));
    event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
    this:inform_current(event);
    return;
  endmethod

  method _display_help_sources owner: ARCH_WIZARD
    "List help sources available in the player's current help environment.";
    env = this:help_environment();
    sources = {};
    for o in (env)
      if (!o.r)
        continue;
      endif
      if (!respond_to(o, 'help_topics))
        continue;
      endif
      name = `o.name ! ANY => ""';
      if (!name)
        continue;
      endif
      sources = {@sources, name};
    endfor
    "De-dup";
    unique = {};
    for name in (sources)
      if (!(name in unique))
        unique = {@unique, name};
      endif
    endfor
    if (!unique)
      return this:inform_current($event:mk_error(this, "No help sources are available here."):with_audience('utility));
    endif
    content = $format.block:mk($format.title:mk("Help Sources"), $format.paragraph:mk("These objects provide help topics in your current context:"), $format.list:mk(unique), $format.paragraph:mk("Try: ", $format.code:inline("help source <name>")));
    event = $event:mk_info(this, content):with_audience('utility):as_inset():with_group('utility, this);
    this:inform_current(event);
    return;
  endmethod

  verb "walk go_to goto" (any any any) owner: ARCH_WIZARD flags: "rxd" [
    ui -> ["arguments" -> ["dobj" -> "Destination"], "label" -> "Go", "prefix" -> "Go to"]
  ]
    "Walk automatically to a destination room.";
    "Usage: walk [to] <destination> | walk stop";
    set_task_perms(player);
    player != this && return;
    set_task_perms(this.owner);
    "Parse destination from argstr - handle 'walk to xxx' and 'walk xxx'";
    dest_str = argstr:trim();
    if (dest_str == "")
      player:inform_current($event:mk_error(player, "Usage: walk [to] <destination> | walk stop"));
      return;
    endif
    "Handle 'walk stop' to cancel";
    if (dest_str == "stop" || dest_str == "cancel")
      canceled = this:action_stop_activities(this, 'walk);
      if (length(canceled) > 0)
        player:inform_current($event:mk_info(player, "You stop walking."));
      else
        player:inform_current($event:mk_info(player, "You aren't walking anywhere."));
      endif
      return;
    endif
    "Strip leading 'to ' if present";
    if (index(dest_str, "to ") == 1)
      dest_str = dest_str[4..$];
    endif
    dest_str = dest_str:trim();
    "Get current room and area";
    current_room = this.location;
    if (!valid(current_room))
      player:inform_current($event:mk_error(player, "You aren't in a room."));
      return;
    endif
    area = current_room.location;
    if (valid(area) && respond_to(area, 'find_passage_by_direction))
      try
        passage = area:find_passage_by_direction(current_room, dest_str);
      except e (ANY)
        player:inform_current($event:mk_error(player, "Error checking passage: " + tostr(e[2])));
        return;
      endtry
      if (typeof(passage) in {TYPE_OBJ, TYPE_FLYWEIGHT})
        try
          traveled = passage:travel_from(this, current_room, ["verb" -> "go", "dobjstr" -> dest_str]);
        except e (ANY)
          player:inform_current($event:mk_error(player, "Error using passage: " + tostr(e[2])));
          return;
        endtry
        return;
      endif
    endif
    if (!valid(area) || !respond_to(area, 'find_path))
      player:inform_current($event:mk_error(player, "You can't navigate from here."));
      return;
    endif
    "Build list of rooms and their names for matching";
    targets = {};
    for room in (area:rooms_from(current_room, false))
      if (!valid(room) || !respond_to(room, 'name))
        continue;
      endif
      room_name = `room:name() ! ANY => ""';
      if (room_name == "")
        continue;
      endif
      targets = {@targets, room};
    endfor
    "Use the same scope matcher as other object commands.";
    match_result = $match:resolve_in_scope(dest_str, targets);
    if (match_result == $ambiguous_match)
      candidates = {room.name for room in ($match:candidates_in_scope(dest_str, targets))};
      if (length(candidates) > 0)
        max = length(candidates) < 5 ? length(candidates) | 5;
        hint = " Try one of: " + candidates[1..max]:join(", ") + ".";
      else
        hint = " Try a more specific room name.";
      endif
      player:inform_current($event:mk_error(player, "\"" + dest_str + "\" is ambiguous." + hint));
      return;
    elseif (match_result == $failed_match || !valid(match_result))
      player:inform_current($event:mk_error(player, "Can't find a place called \"" + dest_str + "\"."));
      return;
    endif
    destination = match_result;
    if (destination == current_room)
      player:inform_current($event:mk_info(player, "You're already at " + destination:noun_phrase('definite, player) + "!"));
      return;
    endif
    "First try to find a walkable route (passages only, no transports)";
    try
      paths = this:_walk_path_options(area, current_room, destination);
    except e (ANY)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return;
    endtry
    {path, path_with_transport} = paths;
    if (!path || length(path) < 2)
      "No passage-only route - check if there's a route with transports";
      if (path_with_transport && length(path_with_transport) >= 2)
        "Route exists but requires transport - find the first transport step";
        for i in [1..length(path_with_transport) - 1]
          {room, connector} = path_with_transport[i];
          if (typeof(connector) == TYPE_LIST && length(connector) >= 1 && connector[1] == 'transport)
            label = connector[2];
            player:inform_current($event:mk_info(player, "To reach " + destination:noun_phrase('definite, player) + ", you'll need to use the exit " + toliteral(label) + ". At " + room:noun_phrase('definite, player) + ", use: " + connector[4] + "."));
            return;
          endif
        endfor
      endif
      player:inform_current($event:mk_error(player, "Can't find a walkable route to " + destination:noun_phrase('definite, player) + "."));
      return;
    endif
    "Cancel any existing movement activity";
    this:action_stop_activities(this, 'walk);
    "Start walking";
    steps = length(path) - 1;
    player:inform_current($event:mk_info(player, "Walking to " + destination:noun_phrase('definite, player) + " (" + tostr(steps) + " " + (steps == 1 ? "step" | "steps") + ")..."));
    "Fork task to do the walking";
    const output_connection = `connection() ! E_INVARG => 0';
    fork activity_task_id (0)
      this:_do_walk(path, output_connection);
    endfork
    this:action_start_activity(this, 'walk, activity_task_id, "walking to " + destination:noun_phrase('definite, player));
  endverb

  method _do_walk owner: ARCH_WIZARD
    "Internal: Execute the walking through a path.";
    "Caller authority is captured by the fork; output remains bound to its initiating connection.";
    caller == this || raise(E_PERM);
    const {path, ?output_connection = 0} = args;
    set_task_perms(caller_perms());
    const current_task = task_id();
    const walk_delay = 2;
    for i in [1..length(path) - 1]
      const {from_room, connector} = path[i];
      const {to_room, _} = path[i + 1];
      "Check player is still in expected room";
      if (this.location != from_room)
        "Player moved manually or was moved - stop walking";
        this:inform_connection(output_connection, $event:mk_info(this, "You've stopped walking (you moved)."));
        this:action_clear_activity_task(this, current_task);
        return;
      endif
      "Check this is a passage (not transport)";
      if (typeof(connector) == TYPE_LIST && connector[1] == 'transport)
        this:inform_connection(output_connection, $event:mk_error(this, "Can't auto-walk through transport - stopping."));
        this:action_clear_activity_task(this, current_task);
        return;
      endif
      "Wait before moving";
      suspend(walk_delay);
      "Check still in expected room after delay";
      if (this.location != from_room)
        this:inform_connection(output_connection, $event:mk_info(this, "You've stopped walking."));
        this:action_clear_activity_task(this, current_task);
        return;
      endif
      "Move via the passage";
      let success;
      try
        success = connector:travel_from(this, from_room, {});
      except e (ANY)
        this:inform_connection(output_connection, $event:mk_error(this, "Walking failed: " + tostr(e[2]) + " - stopping."));
        this:action_clear_activity_task(this, current_task);
        return;
      endtry
      if (!success || this.location != to_room)
        this:inform_connection(output_connection, $event:mk_error(this, "Something blocked your path - stopping."));
        this:action_clear_activity_task(this, current_task);
        return;
      endif
    endfor
    "Arrived at destination";
    const destination = path[$][1];
    this:inform_connection(output_connection, $event:mk_info(this, "You've arrived at " + destination:noun_phrase('definite, player) + "."));
    this:action_clear_activity_task(this, current_task);
  endmethod

  verb "join @join" (any none none) owner: ARCH_WIZARD flags: "rxd"
    "Walk to another player's location.";
    "Usage: join <player>";
    set_task_perms(player);
    player != this && return;
    set_task_perms(this.owner);
    "Parse player name from argstr";
    target_name = argstr:trim();
    if (target_name == "")
      player:inform_current($event:mk_error(player, "Usage: join <player>"));
      return;
    endif
    "Match the player";
    try
      target = $match:match_player(target_name);
    except e (E_INVARG)
      target = $failed_match;
    except e (ANY)
      player:inform_current($event:mk_error(player, "Player lookup failed: " + toliteral(e[2])));
      return;
    endtry
    if (target == $ambiguous_match)
      candidates = {};
      for p in (players())
        if (!valid(p))
          continue;
        endif
        n = `p:name() ! ANY => ""';
        if (n && (index(n, target_name) || index(target_name, n)))
          if (!(n in candidates))
            candidates = {@candidates, n};
          endif
        endif
      endfor
      if (length(candidates) > 0)
        max = length(candidates) < 5 ? length(candidates) | 5;
        hint = " Try one of: " + candidates[1..max]:join(", ") + ".";
      else
        hint = " Try a more specific player name.";
      endif
      player:inform_current($event:mk_error(player, "\"" + target_name + "\" is ambiguous." + hint));
      return;
    elseif (target == $failed_match || !valid(target))
      player:inform_current($event:mk_error(player, "Can't find a player called \"" + target_name + "\"."));
      return;
    endif
    if (target == this)
      player:inform_current($event:mk_info(player, "You can't join yourself!"));
      return;
    endif
    "Check target is in a room";
    target_room = target.location;
    if (!valid(target_room))
      player:inform_current($event:mk_error(player, "You cannot travel to " + target:noun_phrase('definite, player) + "."));
      return;
    endif
    "Get current room and area";
    current_room = this.location;
    if (!valid(current_room))
      player:inform_current($event:mk_error(player, "You aren't in a room."));
      return;
    endif
    if (target_room == current_room)
      player:inform_current($event:mk_info(player, "Already here: " + target:noun_phrase('definite, player) + "."));
      return;
    endif
    area = current_room.location;
    if (!valid(area) || !respond_to(area, 'find_path))
      player:inform_current($event:mk_error(player, "You can't navigate from here."));
      return;
    endif
    "First try to find a walkable route (passages only, no transports)";
    try
      paths = this:_walk_path_options(area, current_room, target_room);
    except e (ANY)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return;
    endtry
    {path, path_with_transport} = paths;
    if (!path || length(path) < 2)
      "No passage-only route - check if there's a route with transports";
      if (path_with_transport && length(path_with_transport) >= 2)
        "Route exists but requires transport - find the first transport step";
        for i in [1..length(path_with_transport) - 1]
          {room, connector} = path_with_transport[i];
          if (typeof(connector) == TYPE_LIST && length(connector) >= 1 && connector[1] == 'transport)
            label = connector[2];
            player:inform_current($event:mk_info(player, "To reach " + target:noun_phrase('definite, player) + ", you'll need to use the exit " + toliteral(label) + ". At " + room:noun_phrase('definite, player) + ", use: " + connector[4] + "."));
            return;
          endif
        endfor
      endif
      player:inform_current($event:mk_error(player, "Can't find a walkable route to " + target:noun_phrase('definite, player) + "."));
      return;
    endif
    "Cancel any existing movement activity";
    this:action_stop_activities(this, 'walk);
    "Start walking";
    steps = length(path) - 1;
    player:inform_current($event:mk_info(player, "Walking to " + target:noun_phrase('definite, player) + " (" + tostr(steps) + " " + (steps == 1 ? "step" | "steps") + ")..."));
    "Fork task to do the walking";
    const output_connection = `connection() ! E_INVARG => 0';
    fork activity_task_id (0)
      this:_do_walk(path, output_connection);
    endfork
    this:action_start_activity(this, 'walk, activity_task_id, "walking to " + target:noun_phrase('definite, player));
  endverb

  method _walk_path_options owner: ARCH_WIZARD
    "Return {passage_only_path, transport_inclusive_path} for walking commands.";
    caller == this || raise(E_PERM);
    set_task_perms(caller_perms());
    {area, current_room, destination} = args;
    try
      path = area:find_path(current_room, destination, true, false);
      path_with_transport = !path || length(path) < 2 ? area:find_path(current_room, destination, true, true) | false;
    except e (ANY)
      raise(E_INVARG, "Route lookup failed: " + toliteral(e));
    endtry
    return {path, path_with_transport};
  endmethod

  method disfunc owner: ARCH_WIZARD
    "Called when player disconnects (after last connection is removed).";
    "last_disconnected is set by #0:user_disconnected before this runs.";
    "Override in subclasses for custom disconnect behavior.";
  endmethod

  method initialize owner: ARCH_WIZARD
    "Set defaults for newly created players.";
    pass();
    set_object_metadata(this, "import_export_hierarchy", {"players"});
  endmethod

  verb home (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Walk automatically to your home room.";
    "Usage: home";
    set_task_perms(player);
    player != this && return;
    "Check if home is set";
    home = this.home;
    if (!valid(home))
      player:inform_current($event:mk_error(player, "You don't have a home set. Usage: @sethome"));
      return;
    endif
    "Check if already at home";
    current_room = this.location;
    if (current_room == home)
      player:inform_current($event:mk_info(player, "You're already home!"));
      return;
    endif
    "Get current area and home's area";
    if (!valid(current_room))
      player:inform_current($event:mk_error(player, "You aren't in a room."));
      return;
    endif
    current_area = current_room.location;
    home_area = home.location;
    "Check if we can navigate";
    if (!valid(current_area) || !respond_to(current_area, 'find_path))
      player:inform_current($event:mk_error(player, "You can't navigate from here."));
      return;
    endif
    "Find a walkable route to home";
    try
      paths = this:_walk_path_options(current_area, current_room, home);
    except e (ANY)
      player:inform_current($event:mk_error(player, tostr(e[2])));
      return;
    endtry
    {path, path_with_transport} = paths;
    if (!path || length(path) < 2)
      "Check if there's a route with transports";
      if (path_with_transport && length(path_with_transport) >= 2)
        "Find the first transport step";
        for i in [1..length(path_with_transport) - 1]
          {room, connector} = path_with_transport[i];
          if (typeof(connector) == TYPE_LIST && length(connector) >= 1 && connector[1] == 'transport)
            label = connector[2];
            player:inform_current($event:mk_info(player, "To reach home, you'll need to use the exit " + toliteral(label) + ". At " + room:noun_phrase('definite, player) + ", use: " + connector[4] + "."));
            return;
          endif
        endfor
      endif
      player:inform_current($event:mk_error(player, "Can't find a walkable route home."));
      return;
    endif
    "Cancel any existing movement activity";
    this:action_stop_activities(this, 'walk);
    "Start walking";
    steps = length(path) - 1;
    player:inform_current($event:mk_info(player, "Walking home to " + home:noun_phrase('definite, player) + " (" + tostr(steps) + " " + (steps == 1 ? "step" | "steps") + ")..."));
    "Fork task to do the walking";
    const output_connection = `connection() ! E_INVARG => 0';
    fork activity_task_id (0)
      this:_do_walk(path, output_connection);
    endfork
    this:action_start_activity(this, 'walk, activity_task_id, "walking home to " + home:noun_phrase('definite, player));
  endverb

  verb "@sethome" (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Set your current location as your home room.";
    set_task_perms(player);
    player != this && return;
    current_room = this.location;
    if (!valid(current_room))
      player:inform_current($event:mk_error(player, "You aren't in a valid room."));
      return;
    endif
    "Check it's actually a room";
    if (!isa(current_room, $room))
      player:inform_current($event:mk_error(player, "You can only set a room as your home."));
      return;
    endif
    old_home = this.home;
    this.home = current_room;
    if (valid(old_home) && old_home != current_room)
      player:inform_current($event:mk_info(player, "Home set to " + current_room:noun_phrase('definite, player) + " (was " + old_home:noun_phrase('definite, player) + ")."));
    else
      player:inform_current($event:mk_info(player, "Home set to " + current_room:noun_phrase('definite, player) + "."));
    endif
  endverb

  method input_placeholder owner: ARCH_WIZARD
    "Return placeholder text for the input field.";
    "Can be overridden to provide context-sensitive hints.";
    "Checks room's input_placeholders verb first.";
    loc = this.location;
    if (valid(loc))
      room_placeholders = `loc:input_placeholders() ! ANY => false';
      if (typeof(room_placeholders) == TYPE_LIST && length(room_placeholders) > 0)
        return room_placeholders[random(length(room_placeholders))];
      endif
    endif
    "Default placeholders";
    placeholders = {"What would you like to explore?", "Ready for your next adventure?", "What's on your mind?", "How can we help you today?", "What would you like to try?", "Share your thoughts...", "What's your next move?", "Ready to discover something new?"};
    return placeholders[random(length(placeholders))];
  endmethod

  method has_admin_elevation owner: ARCH_WIZARD
    "True when this player is running inside delegated admin elevation.";
    const principal = caller_perms();
    principal == this || (valid(principal) && principal.wizard) || raise(E_PERM);
    admin_features = this.admin_features;
    set_task_perms(this);
    typeof(admin_features) == TYPE_LIST || raise(E_TYPE, "player.admin_features must be a list");
    for feat in (admin_features)
      if (!valid(feat))
        continue;
      endif
      if (feat:is_elevated(this))
        return true;
      endif
    endfor
    return false;
  endmethod

  verb assist (any any any) owner: ARCH_WIZARD flags: "rd"
    "Run LLM assistance for this connection's last request, an explicit request token, or free text.";
    player == this || raise(E_PERM);
    const destination = `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return false;
    const input = argstr:trim();
    length(input) <= 1024 || raise(E_QUOTA, "Assist input is too long.");
    set_task_perms(this);
    if (input == "--request")
      return this:inform_connection(destination,
        $event:mk_info(this, "Usage: assist --request TOKEN"):with_audience('utility));
    endif
    if (input != "" && !input:starts_with("--request "))
      const pc = ['verb -> input, 'dobjstr -> "", 'prepstr -> "", 'iobjstr -> ""];
      return this:_assist_with_pc(pc, destination);
    endif
    const token = input == "" ? "" | input[11..$]:trim();
    const context = $llm_assist_context:take(this, destination, token);
    if (typeof(context) != TYPE_FLYWEIGHT)
      this:inform_connection(destination, $event:mk_info(this,
        "No current assist request on this connection. Try your command or help query again."):with_audience('utility));
      return true;
    endif
    if (context.kind == 'help)
      return this:_assist_with_help_query(context.input, destination);
    endif
    return this:_assist_with_pc(context.input, destination);
  endverb

  method _assist_with_pc owner: ARCH_WIZARD
    "Start a bounded LLM command suggestion using the invoking actor and connection.";
    $llm_tool_provider:_require_tool_dispatch(this, caller_perms());
    const {pc, ?requested_connection = 0} = args;
    const destination = typeof(requested_connection) == TYPE_OBJ
      ? requested_connection | `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return false;
    const client = $player.suggestions_llm_client;
    set_task_perms(this);
    if (typeof(client) != TYPE_OBJ || !valid(client))
      this:inform_connection(destination,
        $event:mk_info(this, "Assist is not configured right now."):with_audience('utility));
      return false;
    endif
    let request = [];
    for field in ({'verb, 'dobjstr, 'prepstr, 'iobjstr})
      request[field] = pc[field];
    endfor
    return $llm_command_suggestion:offer(this, destination, request);
  endmethod

  method _assist_with_help_query owner: ARCH_WIZARD
    "Start an owned LLM help suggestion after explicit assist opt-in.";
    $llm_tool_provider:_require_tool_dispatch(this, caller_perms());
    const {query, ?requested_connection = 0} = args;
    const destination = typeof(requested_connection) == TYPE_OBJ
      ? requested_connection | `connection() ! E_INVARG => 0';
    typeof(destination) == TYPE_OBJ || return false;
    set_task_perms(this);
    const handled = $llm_help_suggestion:offer(this, destination, ['query -> query]);
    if (!handled)
      this:inform_connection(destination,
        $event:mk_error(this, "Help assist is not configured right now."):with_audience('utility));
      return false;
    endif
    return true;
  endmethod

  verb stop (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Stop active background activities.";
    set_task_perms(player);
    if (player != this)
      return;
    endif
    const suggestions = $llm_suggestion:cancel_all(this);
    set_task_perms(this.owner);
    canceled = this:action_stop_activities(this);
    if (suggestions > 0)
      canceled = {@canceled, ['kind -> 'llm_suggestion, 'task_id -> 0, 'label -> "LLM suggestions"]};
    endif
    count = length(canceled);
    if (count == 0)
      player:inform_current($event:mk_info(player, "You're not doing anything."));
      return;
    endif
    if (count == 1)
      description = $player_activity:description_of(canceled[1]);
      if (index(description, "walking") == 1)
        player:inform_current($event:mk_info(player, "You stop walking."));
      else
        player:inform_current($event:mk_info(player, "You stop " + description + "."));
      endif
      return;
    endif
    descriptions = {};
    for entry in (canceled)
      description = $player_activity:description_of(entry);
      if (!(description in descriptions))
        descriptions = {@descriptions, description};
      endif
    endfor
    if (length(descriptions) == 1)
      desc = descriptions[1];
      if (index(desc, "walking") == 1)
        player:inform_current($event:mk_info(player, "You stop walking."));
      else
        player:inform_current($event:mk_info(player, "You stop " + desc + "."));
      endif
    else
      max = length(descriptions) < 3 ? length(descriptions) | 3;
      sample = descriptions[1..max]:join(", ");
      player:inform_current($event:mk_info(player, "You stop " + tostr(count) + " activities (" + sample + ")."));
    endif
  endverb

  method inspection_actions owner: ARCH_WIZARD
    "Suggest examination and a direct message through the ordinary command parser.";
    const {?who = player} = args;
    const ref = tostr(this);
    const actions = {["id" -> "examine", "label" -> "Examine", "command" -> "examine " + ref, "action" -> $player:command_action("examine", this:name())]};
    who == this && return actions;
    return {@actions, ["id" -> "message", "label" -> "Send message", "command" -> "dm " + ref + " {input}", "action" -> $player:command_action("dm", this:name()), "input" -> ["label" -> "Message", "placeholder" -> "Write a private message"]]};
  endmethod

  method test_help_environment_includes_global_player_and_features owner: HACKER
    "Help lookup should cover global topics, the player surface, and feature-generated topics.";
    "Exercise the command surface as its owning player, not the player prototype.";
    const subject = $hacker;
    const denied = `$player:command_environment() ! E_PERM => E_PERM';
    $test_utils:assert_eq(denied, E_PERM, "Unrelated callers cannot inspect another command environment");
    help_env = subject:help_environment();
    command_env = subject:command_environment();
    $test_utils:assert_true(is_member(subject, help_env), "help environment should include player");
    $test_utils:assert_true(is_member(subject, command_env), "command environment should include player");
    is_builder = `subject.is_builder ! ANY => false';
    if (is_builder)
      $test_utils:assert_true(is_member($builder_features, help_env), "builder help should include builder features");
      $test_utils:assert_true(is_member($builder_features, command_env), "builder command environment should include builder features");
    endif
    say_topic = subject:find_help_topic("say");
    $test_utils:assert_type(say_topic, TYPE_FLYWEIGHT, "global help topic 'say' should resolve");
    $test_utils:assert_eq(say_topic.name, "say", "global help topic name");
    emote_topic = subject:find_help_topic("emote");
    $test_utils:assert_type(emote_topic, TYPE_FLYWEIGHT, "global help topic 'emote' should resolve");
    $test_utils:assert_eq(emote_topic.name, "emote", "global emote help topic name");
    bonk_topic = subject:find_help_topic("bonk");
    $test_utils:assert_type(bonk_topic, TYPE_FLYWEIGHT, "feature-generated help topic 'bonk' should resolve");
    $test_utils:assert_eq(bonk_topic.name, "bonk", "feature-generated help topic name");
    return true;
  endmethod

  method test_targeted_help_uses_requested_object owner: HACKER
    "Targeted object help should inspect the requested object instead of dropping it.";
    scratch = $test_utils:anonymous($thing);
    add_property(scratch, 'object_help, "Targeted help body.", {scratch.owner, "r"});
    result = this:_show_targeted_help(scratch);
    $test_utils:assert_type(result, TYPE_LIST, "targeted help should return a result tuple");
    $test_utils:assert_eq(length(result), 2, "targeted help should return {lines, has_doc}");
    $test_utils:assert_true(result[2], "targeted help should report object documentation");
    $test_utils:assert_true(length(result[1]) > 0, "targeted help should include display lines");
    return true;
  endmethod

  method test_look_passage_reports_malformed_passage owner: ARCH_WIZARD
    "Passage lookup should report malformed passage records instead of treating them as misses.";
    original_location = this.location;
    area = $area:create(true);
    room_a = $room:create(true);
    room_b = $room:create(true);
    const bad_tuple = room_a < room_b ? {room_a, room_b, $thing} | {room_b, room_a, $thing};
    try
      move(room_a, area);
      move(room_b, area);
      move(this, room_a);
      area.passages_rel:assert(bad_tuple);
      room_a.passage_areas = {area};
      room_b.passage_areas = {area};
      raised = false;
      try
        this:_look_passage("north");
      except e (E_INVARG)
        raised = true;
      endtry
      $test_utils:assert_true(raised, "malformed passage records should raise");
    finally
      area.passages_rel:retract(bad_tuple);
      valid(original_location) && move(this, original_location);
      $test_utils:destroy_if_valid(room_b);
      $test_utils:destroy_if_valid(room_a);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod

  method test_walk_path_options_reports_malformed_route owner: ARCH_WIZARD
    "Walking route lookup should report malformed passage records instead of treating them as no route.";
    area = $area:create(true);
    room_a = $room:create(true);
    room_b = $room:create(true);
    const bad_tuple = room_a < room_b ? {room_a, room_b, $thing} | {room_b, room_a, $thing};
    try
      move(room_a, area);
      move(room_b, area);
      area.passages_rel:assert(bad_tuple);
      room_a.passage_areas = {area};
      room_b.passage_areas = {area};
      raised = false;
      try
        this:_walk_path_options(area, room_a, room_b);
      except e (E_INVARG)
        raised = true;
      endtry
      $test_utils:assert_true(raised, "malformed route records should raise");
    finally
      area.passages_rel:retract(bad_tuple);
      $test_utils:destroy_if_valid(room_b);
      $test_utils:destroy_if_valid(room_a);
      $test_utils:destroy_if_valid(area);
    endtry
    return true;
  endmethod

  method test_format_gag_entry_reports_invalid_entries owner: ARCH_WIZARD
    "Gag-list display should show malformed entries instead of silently dropping them.";
    formatted = this:_format_gag_entry(#-1);
    $test_utils:assert_true(index(formatted, "invalid entry") > 0, "invalid gag-list entry should be visible");
    return true;
  endmethod
endobject
