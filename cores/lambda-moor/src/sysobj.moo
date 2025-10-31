object SYSOBJ
  name: "The System Object"
  parent: ROOT_CLASS
  owner: #2
  readable: true

  property ambiguous_match (owner: #2, flags: "rc") = #-2;
  property big_mail_recipient (owner: #2, flags: "rc") = BIG_MAIL_RECIPIENT;
  property biglist (owner: #2, flags: "rc") = BIGLIST;
  property build_options (owner: #2, flags: "rc") = BUILD_OPTIONS;
  property builder (owner: #2, flags: "rc") = BUILDER;
  property builder_help (owner: #2, flags: "r") = BUILDER_HELP;
  property building_utils (owner: #2, flags: "rc") = BUILDING_UTILS;
  property builtin_function_help (owner: #2, flags: "rc") = BUILTIN_FUNCTION_HELP;
  property byte_quota_utils (owner: #2, flags: "rc") = BYTE_QUOTA_UTILS;
  property class_registry (owner: #2, flags: "rc") = {
    {
      "generics",
      "Generic objects intended for use as the parents of new objects",
      {
        ROOM,
        EXIT,
        THING,
        NOTE,
        LETTER,
        CONTAINER,
        ROOT_CLASS,
        PLAYER,
        PROG,
        WIZ,
        GENERIC_EDITOR,
        MAIL_RECIPIENT,
        MAIL_AGENT
      }
    },
    {
      "utilities",
      "Objects holding useful general-purpose verbs",
      {
        STRING_UTILS,
        LIST_UTILS,
        WIZ_UTILS,
        SET_UTILS,
        GENDER_UTILS,
        MATH_UTILS,
        TIME_UTILS,
        MATCH_UTILS,
        OBJECT_UTILS,
        LOCK_UTILS,
        COMMAND_UTILS,
        PERM_UTILS,
        BUILDING_UTILS,
        SEQ_UTILS,
        BIGLIST,
        BYTE_QUOTA_UTILS,
        OBJECT_QUOTA_UTILS,
        CODE_UTILS,
        MATRIX_UTILS,
        CONVERT_UTILS
      }
    }
  };
  property code_utils (owner: #2, flags: "rc") = CODE_UTILS;
  property command_utils (owner: #2, flags: "rc") = COMMAND_UTILS;
  property container (owner: #2, flags: "rc") = CONTAINER;
  property convert_utils (owner: #2, flags: "rc") = CONVERT_UTILS;
  property core_help (owner: #2, flags: "rc") = CORE_HELP;
  property core_history (owner: #2, flags: "r") = {{"LambdaMOO", "1.8.3+47", 1529447738}};
  property display_options (owner: #2, flags: "rc") = DISPLAY_OPTIONS;
  property dump_interval (owner: #2, flags: "rc") = 3600;
  property edit_options (owner: #2, flags: "rc") = EDIT_OPTIONS;
  property editor_help (owner: #2, flags: "rc") = EDITOR_HELP;
  property error (owner: #2, flags: "rc") = ERROR;
  property exit (owner: #2, flags: "rc") = EXIT;
  property failed_match (owner: #2, flags: "rc") = #-3;
  property feature (owner: #2, flags: "rc") = FEATURE;
  property feature_warehouse (owner: #2, flags: "r") = FEATURE_WAREHOUSE;
  property force_input_count (owner: #2, flags: "rc") = 19398082;
  property frand_class (owner: #2, flags: "rc") = FRAND_CLASS;
  property frand_help (owner: #2, flags: "rc") = FRAND_HELP;
  property ftp (owner: #2, flags: "rc") = FTP;
  property garbage (owner: #2, flags: "rc") = GARBAGE;
  property gender_utils (owner: #2, flags: "r") = GENDER_UTILS;
  property gendered_object (owner: #2, flags: "r") = GENDERED_OBJECT;
  property generic_biglist_home (owner: #2, flags: "r") = GENERIC_BIGLIST_HOME;
  property generic_db (owner: #2, flags: "rc") = GENERIC_DB;
  property generic_editor (owner: #2, flags: "rc") = GENERIC_EDITOR;
  property generic_help (owner: #2, flags: "rc") = GENERIC_HELP;
  property generic_options (owner: #2, flags: "rc") = GENERIC_OPTIONS;
  property generic_utils (owner: #2, flags: "rc") = GENERIC_UTILS;
  property gopher (owner: #2, flags: "r") = GOPHER;
  property gripe_recipients (owner: #2, flags: "rc") = {#2};
  property guest (owner: #2, flags: "r") = GUEST;
  property guest_log (owner: #2, flags: "r") = GUEST_LOG;
  property hacker (owner: #2, flags: "rc") = HACKER;
  property help (owner: #2, flags: "rc") = HELP;
  property housekeeper (owner: #2, flags: "rc") = HOUSEKEEPER;
  property http (owner: #2, flags: "rc") = HTTP;
  property last_huh (owner: #2, flags: "r") = LAST_HUH;
  property last_restart_time (owner: #2, flags: "rc") = 1529543472;
  property letter (owner: #2, flags: "rc") = LETTER;
  property limbo (owner: #2, flags: "rc") = LIMBO;
  property list_editor (owner: #2, flags: "rc") = LIST_EDITOR;
  property list_utils (owner: #2, flags: "rc") = LIST_UTILS;
  property local (owner: #2, flags: "rc") = #-1;
  property lock_utils (owner: #2, flags: "rc") = LOCK_UTILS;
  property login (owner: #2, flags: "r") = LOGIN;
  property mail_agent (owner: #2, flags: "rc") = MAIL_AGENT;
  property mail_editor (owner: #2, flags: "rc") = MAIL_EDITOR;
  property mail_help (owner: #2, flags: "r") = MAIL_HELP;
  property mail_options (owner: #2, flags: "rc") = MAIL_OPTIONS;
  property mail_recipient (owner: #2, flags: "rc") = MAIL_RECIPIENT;
  property mail_recipient_class (owner: #2, flags: "rc") = MAIL_RECIPIENT_CLASS;
  property match_utils (owner: #2, flags: "rc") = MATCH_UTILS;
  property math_utils (owner: #2, flags: "rc") = MATH_UTILS;
  property matrix_utils (owner: HACKER, flags: "r") = MATRIX_UTILS;
  property max_seconds (owner: #2, flags: "rc") = 5;
  property max_ticks (owner: #2, flags: "rc") = 30000;
  property maxint (owner: #2, flags: "rc") = 2147483647;
  property minint (owner: #2, flags: "rc") = -2147483648;
  property network (owner: #2, flags: "rc") = NETWORK;
  property new_player_log (owner: #2, flags: "rc") = NEW_PLAYER_LOG;
  property new_prog_log (owner: #2, flags: "rc") = NEW_PROG_LOG;
  property news (owner: #2, flags: "rc") = NEWS;
  property newt_log (owner: #2, flags: "rc") = NEWT_LOG;
  property no_connect_message (owner: #2, flags: "rc") = 0;
  property no_one (owner: #2, flags: "r") = NO_ONE;
  property note (owner: #2, flags: "rc") = NOTE;
  property note_editor (owner: #2, flags: "rc") = NOTE_EDITOR;
  property nothing (owner: #2, flags: "rc") = #-1;
  property object_quota_utils (owner: #2, flags: "rc") = OBJECT_QUOTA_UTILS;
  property object_utils (owner: #2, flags: "rc") = OBJECT_UTILS;
  property paranoid_db (owner: #2, flags: "r") = PARANOID_DB;
  property password_verifier (owner: #2, flags: "r") = PASSWORD_VERIFIER;
  property pasting_feature (owner: #2, flags: "rc") = PASTING_FEATURE;
  property perm_utils (owner: #2, flags: "rc") = PERM_UTILS;
  property player (owner: #2, flags: "rc") = PLAYER;
  property player_class (owner: #2, flags: "rc") = MAIL_RECIPIENT_CLASS;
  property player_db (owner: #2, flags: "r") = PLAYER_DB;
  property player_start (owner: #2, flags: "rc") = PLAYER_START;
  property prog (owner: #2, flags: "rc") = PROG;
  property prog_help (owner: #2, flags: "rc") = PROG_HELP;
  property prog_options (owner: #2, flags: "rc") = PROG_OPTIONS;
  property quota_log (owner: #2, flags: "rc") = QUOTA_LOG;
  property quota_utils (owner: #2, flags: "rc") = BYTE_QUOTA_UTILS;
  property recycler (owner: #2, flags: "rc") = RECYCLER;
  property registration_db (owner: #2, flags: "rc") = REGISTRATION_DB;
  property room (owner: #2, flags: "rc") = ROOM;
  property root_class (owner: #2, flags: "rc") = ROOT_CLASS;
  property seq_utils (owner: #2, flags: "rc") = SEQ_UTILS;
  property server_options (owner: #2, flags: "rc") = SERVER_OPTIONS;
  property set_utils (owner: #2, flags: "rc") = SET_UTILS;
  property shutdown_message (owner: #2, flags: "rc") = "";
  property shutdown_task (owner: #2, flags: "rc") = E_NONE;
  property shutdown_time (owner: #2, flags: "rc") = 0;
  property site_db (owner: #2, flags: "rc") = SITE_DB;
  property site_log (owner: #2, flags: "rc") = NEWT_LOG;
  property spell (owner: #2, flags: "rc") = SPELL;
  property stage_talk (owner: #2, flags: "rc") = STAGE_TALK;
  property string_utils (owner: #2, flags: "rc") = STRING_UTILS;
  property sysobj (owner: #2, flags: "r") = SYSOBJ;
  property thing (owner: #2, flags: "rc") = THING;
  property time_utils (owner: #2, flags: "rc") = TIME_UTILS;
  property toad_log (owner: #2, flags: "rc") = NEWT_LOG;
  property trig_utils (owner: #2, flags: "rc") = MATH_UTILS;
  property verb_editor (owner: #2, flags: "rc") = VERB_EDITOR;
  property verb_help (owner: #2, flags: "rc") = VERB_HELP;
  property wiz (owner: #2, flags: "rc") = WIZ;
  property wiz_help (owner: #2, flags: "rc") = WIZ_HELP;
  property wiz_utils (owner: #2, flags: "rc") = WIZ_UTILS;
  property you (owner: HACKER, flags: "r") = YOU;

  override aliases = {"The System Object"};
  override description = "The known universe.";
  override import_export_id = "sysobj";
  override object_size = {23528, 1084848672};

  verb do_login_command (this none this) owner: #2 flags: "rxd"
    "...This code should only be run as a server task...";
    if (callers())
      return E_PERM;
    endif
    if (typeof(h = $network:incoming_connection(player)) == OBJ)
      "connected to an object";
      return h;
    elseif (h)
      return 0;
    endif
    host = $string_utils:connection_hostname(connection_name(player));
    if ($login:redlisted(host))
      boot_player(player);
      server_log(tostr("REDLISTED: ", player, " from ", host));
      return 0;
    endif
    "HTTP server by Krate";
    try
      newargs = $http:handle_connection(@args);
      if (!newargs)
        return 0;
      endif
      args = newargs;
    except v (ANY)
    endtry
    "...checks to see if the login is spamming the server with too many commands...";
    if (!$login:maybe_limit_commands())
      args = $login:parse_command(@args);
      return $login:((args[1]))(@listdelete(args, 1));
    endif
  endverb

  verb server_started (this none this) owner: #2 flags: "rxd"
    if (!callers())
      $last_restart_time = time();
      $network:server_started();
      $login:server_started();
    endif
  endverb

  verb "core_object_info core_objects" (this none this) owner: #2 flags: "rxd"
    set_task_perms($no_one);
    {?core_variant = "Imnotsurewhatthisshouldbeyetdontdependonthis", ?in_mcd = 0} = args;
    if (in_mcd)
      {vb, perms, loc} = (callers()[1])[2..4];
      if (vb != "make-core-database" || !perms.wizard || loc != $wiz)
        raise(E_PERM);
      endif
    endif
    core_objects = {};
    proxy_original = proxy_incore = core_properties = skipped_parents = {};
    todo = {{#0, {"sysobj", "owner"}}};
    "...lucky for us #0 has a self-referential property";
    while ({?sfc, @todo} = todo)
      {o, ?props_to_follow = {}} = sfc;
      o_props = {};
      for p in (props_to_follow)
        v = o.(p);
        if (typeof(v) != OBJ || !valid(v))
          continue p;
        endif
        o_props = {@o_props, p};
        if (v in proxy_original || v in core_objects)
          "...we have been here before...";
          continue p;
        endif
        if ($object_utils:has_callable_verb(v, "proxy_for_core"))
          "...proxy_for_core() returns an object to";
          "...take the place of v in the final core.";
          proxy_original[1..0] = {v};
          try
            vnew = v:proxy_for_core(core_variant, in_mcd);
            if (typeof(vnew) != OBJ)
              raise(E_TYPE, "returned non-object");
            elseif (vnew in proxy_original > 1)
              raise(E_RECMOVE, "proxy loop");
            endif
          except e (ANY)
            player:notify(tostr("Error from ", v, ":proxy_for_core => ", e[2]));
            player:notify(toliteral(e[4]));
            vnew = #-1;
          endtry
          if (vnew == v)
            proxy_original[1..1] = {};
          else
            proxy_incore[1..0] = {vnew};
            if (vnew in core_objects || !valid(vnew))
              continue p;
            endif
            v = vnew;
          endif
        endif
        if ($object_utils:has_callable_verb(v, "include_for_core"))
          "...include_for_core() returns a list of properties on v";
          "...to be searched for additional core objects.";
          try
            v_props = v:include_for_core(core_variant);
            if (typeof(v_props) != LIST)
              raise(E_TYPE, "returned non-list");
            endif
            if (v_props)
              todo = {@todo, {v, v_props}};
            endif
          except e (ANY)
            player:notify(tostr("Error from ", v, ":include_for_core => ", e[2]));
            player:notify(toliteral(e[4]));
          endtry
        endif
        core_objects = setadd(core_objects, v);
      endfor
      core_properties = {@core_properties, {o, o_props}};
    endwhile
    for o in (core_objects)
      p = parent(o);
      while (valid(p))
        if (!(p in core_objects))
          skipped_parents = setadd(skipped_parents, p);
        endif
        p = parent(p);
      endwhile
    endfor
    if (verb == "core_object_info")
      "... what make-core-database needs";
      return {core_objects, core_properties, skipped_parents, proxy_original, proxy_incore};
    else
      "... what most people care about";
      return core_objects;
    endif
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      `delete_property(this, "mail_name_db") ! E_PROPNF';
      `delete_verb(this, "do_command") ! E_VERBNF';
      $core_history = {{$network.MOO_name, server_version(), time()}, @$core_history};
      $shutdown_message = "";
      $shutdown_time = 0;
      $dump_interval = 3600;
      $gripe_recipients = {player};
      $class_registry = {{"generics", "Generic objects intended for use as the parents of new objects", {$room, $exit, $thing, $note, $letter, $container, $root_class, $player, $prog, $wiz, $generic_editor, $mail_recipient, $mail_agent}}, {"utilities", "Objects holding useful general-purpose verbs", children($generic_utils)}};
      for v in ({"do_login_command", "server_started"})
        c = {};
        for i in (verb_code(this, v))
          c = {@c, strsub(i, "$local.login", "$login")};
        endfor
        set_verb_code(#0, v, c);
      endfor
    endif
  endverb

  verb "user_created user_connected" (this none this) owner: #2 flags: "rxd"
    "Copied from The System Object (#0):user_connected by Slartibartfast (#4242) Sun May 21 18:14:16 1995 PDT";
    if (callers())
      return;
    endif
    user = args[1];
    set_task_perms(user);
    try
      user.location:confunc(user);
      user:confunc();
    except id (ANY)
      user:tell("Confunc failed: ", id[2], ".");
      for tb in (id[4])
        user:tell("... called from ", tb[4], ":", tb[2], tb[4] != tb[1] ? tostr(" (this == ", tb[1], ")") | "", ", line ", tb[6]);
      endfor
      user:tell("(End of traceback)");
    endtry
  endverb

  verb "user_disconnected user_client_disconnected" (this none this) owner: #2 flags: "rxd"
    if (callers())
      return;
    endif
    if (args[1] < #0)
      "not logged in user.  probably should do something clever here involving Carrot's no-spam hack.  --yduJ";
      return;
    endif
    user = args[1];
    user.last_disconnect_time = time();
    set_task_perms(user);
    where = user.location;
    `user:disfunc() ! ANY => 0';
    if (user.location != where)
      `where.location:disfunc(user) ! ANY => 0';
    endif
    `user.location:disfunc(user) ! ANY => 0';
  endverb

  verb "bf_chparent chparent" (this none this) owner: #2 flags: "rxd"
    "chparent(object, new-parent) -- see help on the builtin.";
    who = caller_perms();
    {what, papa} = args;
    if (typeof(what) != OBJ)
      retval = E_TYPE;
    elseif (!valid(what))
      retval = E_INVARG;
    elseif (typeof(papa) != OBJ)
      retval = E_TYPE;
    elseif (!valid(papa) && papa != #-1)
      retval = E_INVIND;
    elseif (!$perm_utils:controls(who, what))
      retval = E_PERM;
    elseif (is_player(what) && !$object_utils:isa(papa, $player_class) && !who.wizard)
      retval = E_PERM;
    elseif (is_player(what) && !$object_utils:isa(what, $player_class) && !who.wizard)
      retval = E_PERM;
    elseif (children(what) && $object_utils:isa(what, $player_class) && !$object_utils:isa(papa, $player_class))
      retval = E_PERM;
    elseif (is_player(what) && what in $wiz_utils.chparent_restricted && !who.wizard)
      retval = E_PERM;
    elseif (what.location == $mail_agent && $object_utils:isa(what, $mail_recipient) && !$object_utils:isa(papa, $mail_recipient) && !who.wizard)
      retval = E_PERM;
    elseif (!valid(papa) || ($perm_utils:controls(who, papa) || papa.f))
      retval = `chparent(@args) ! ANY';
    else
      retval = E_PERM;
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb "bf_add_verb add_verb" (this none this) owner: #2 flags: "rxd"
    "add_verb() -- see help on the builtin for more information. This verb is called by the server when $server_options.protect_add_verb exists and is true and caller_perms() are not wizardly.";
    who = caller_perms();
    what = args[1];
    info = args[2];
    if (typeof(what) != OBJ)
      retval = E_TYPE;
    elseif (!valid(what))
      retval = E_INVARG;
    elseif (!$perm_utils:controls(who, what) && !what.w)
      "caller_perms() is not allowed to hack on the object in question";
      retval = E_PERM;
    elseif (!$perm_utils:controls(who, info[1]))
      "caller_perms() is not permitted to add a verb with the specified owner.";
      retval = E_PERM;
    elseif (index(info[2], "w") && !$server_options.permit_writable_verbs)
      retval = E_INVARG;
    elseif (!$quota_utils:verb_addition_permitted(who))
      retval = E_QUOTA;
    elseif (what.owner != who && !who.wizard && !$quota_utils:verb_addition_permitted(what.owner))
      retval = E_QUOTA;
    elseif (!who.programmer)
      retval = E_PERM;
    else
      "we now know that the caller's perms control the object or the object is writable, and we know that the caller's perms control the prospective verb owner (by more traditional means)";
      retval = `add_verb(@args) ! ANY';
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb "bf_add_property add_property" (this none this) owner: #2 flags: "rxd"
    "add_property() -- see help on the builtin for more information. This verb is called by the server when $server_options.protect_add_property exists and is true and caller_perms() are not wizardly.";
    who = caller_perms();
    {what, propname, value, info} = args;
    if (typeof(what) != OBJ)
      retval = E_TYPE;
    elseif (!valid(what))
      retval = E_INVARG;
    elseif (!$perm_utils:controls(who, what) && !what.w)
      retval = E_PERM;
    elseif (!$perm_utils:controls(who, info[1]))
      retval = E_PERM;
    elseif (!$quota_utils:property_addition_permitted(who))
      retval = E_QUOTA;
    elseif (what.owner != who && !who.wizard && !$quota_utils:property_addition_permitted(what.owner))
      retval = E_QUOTA;
      "elseif (!who.programmer)";
      "  return E_PERM;     I wanted to do this, but $builder:@newmessage relies upon nonprogs being able to call add_property.  --Nosredna";
    elseif (propname in {"object_size", "size_quota", "queued_task_limit"} && !who.wizard)
      retval = E_PERM;
    else
      "we now know that the caller's perms control the object (or the object is writable), and that the caller's perms are permitted to control the new property's owner.";
      retval = `add_property(@args) ! ANY';
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb "bf_recycle recycle" (this none this) owner: #2 flags: "rxd"
    "recycle(object) -- see help on the builtin. This verb is called by the server when $server_options.protect_recycle exists and is true and caller_perms() are not wizardly.";
    if (!valid(what = args[1]))
      retval = E_INVARG;
    elseif (!$perm_utils:controls(who = caller_perms(), what))
      retval = E_PERM;
    elseif ((p = is_player(what)) && !who.wizard)
      for p in ($wiz_utils:connected_wizards_unadvertised())
        p:tell($string_utils:pronoun_sub("%N (%#) is currently trying to recycle %t (%[#t])", who, what));
      endfor
      retval = E_PERM;
    else
      if (p)
        $wiz_utils:unset_player(what);
      endif
      $recycler:kill_all_tasks(what);
      retval = `recycle(what) ! ANY';
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb user_reconnected (this none this) owner: #2 flags: "rxd"
    if (callers())
      return;
    endif
    if ($object_utils:isa(user = args[1], $guest))
      "from $guest:boot";
      oldloc = user.location;
      move(user, $nothing);
      "..force enterfunc to be called so that the newbie gets a room description.";
      move(user, user.home);
      user:do_reset();
      if ($object_utils:isa(oldloc, $room))
        oldloc:announce("In the distance you hear someone's alarm clock going off.");
        if (oldloc != user.location)
          oldloc:announce(user.name, " wavers and vanishes into insubstantial mist.");
        else
          oldloc:announce(user.name, " undergoes a wrenching personality shift.");
        endif
      endif
      set_task_perms(user);
      `user:confunc() ! ANY';
    endif
  endverb

  verb "bf_set_verb_info set_verb_info" (this none this) owner: #2 flags: "rxd"
    "set_verb_info() -- see help on the builtin for more information. This verb is called by the server when $server_options.protect_set_verb_info exists and is true and caller_perms() are not wizardly.";
    {o, v, i} = args;
    if (typeof(vi = `verb_info(o, v) ! ANY') == ERR)
      "probably verb doesn't exist";
      retval = vi;
    elseif (!$perm_utils:controls(cp = caller_perms(), vi[1]))
      "perms don't control the current verb owner";
      retval = E_PERM;
    elseif (typeof(i) != LIST || typeof(no = i[1]) != OBJ)
      "info is malformed";
      retval = E_TYPE;
    elseif (!valid(no) || !is_player(no))
      "invalid new verb owner";
      retval = E_INVARG;
    elseif (!$perm_utils:controls(cp, no))
      "perms don't control prospective verb owner";
      retval = E_PERM;
    elseif (index(i[2], "w") && !`$server_options.permit_writable_verbs ! E_PROPNF, E_INVIND => 1')
      retval = E_INVARG;
    else
      retval = `set_verb_info(o, v, i) ! ANY';
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb "bf_match match" (this none this) owner: #2 flags: "rxd"
    m = `match(@args) ! ANY';
    return typeof(m) == ERR && $code_utils:dflag_on() ? raise(m) | m;
    if (length(args[1]) > 256 && index(args[2], "*"))
      return E_INVARG;
    else
      return match(@args);
    endif
  endverb

  verb "bf_rmatch rmatch" (this none this) owner: #2 flags: "rxd"
    r = `rmatch(@args) ! ANY';
    return typeof(r) == ERR && $code_utils:dflag_on() ? raise(r) | r;
    if (length(args[1]) > 256 && index(args[2], "*"))
      return E_INVARG;
    else
      return rmatch(@args);
    endif
  endverb

  verb checkpoint_finished (this none this) owner: #2 flags: "rxd"
    "Copied from The System Object (#0):checkpoint_finished [verb author Heathcliff (#89987)] at Fri May  7 12:02:22 2004 PDT";
    callers() && raise(E_PERM);
    $login.checkpoint_in_progress = 0;
    `$local.checkpoint_notification:checkpoint_finished(@args) ! ANY';
  endverb

  verb "do_out_of_band_command doobc" (this none this) owner: #2 flags: "rxd"
    "do_out_of_band_command -- a cheap and very dirty do_out_of_band verb.  Forwards to verb on player with same name if it exists, otherwise forwards to $login.  May only be called by the server in response to an out of band command, otherwise E_PERM is returned.";
    if (caller == #-1 && caller_perms() == #-1 && callers() == {})
      if (valid(player) && is_player(player))
        set_task_perms(player);
        $object_utils:has_callable_verb(player, "do_out_of_band_command") && player:do_out_of_band_command(@args);
      else
        $login:do_out_of_band_command(@args);
      endif
    else
      return E_PERM;
    endif
  endverb

  verb handle_uncaught_error (this none this) owner: #2 flags: "rxd"
    if (!callers())
      {code, msg, value, stack, traceback} = args;
      if (!$object_utils:connected(player))
        "Mail the player the traceback if e isn't connected.";
        $mail_agent:send_message(#0, player, {"traceback", $gripe_recipients}, traceback);
      endif
      "now let the player do something with it if e wants...";
      return `player:(verb)(@args) ! ANY';
    endif
  endverb

  verb checkpoint_started (this none this) owner: #2 flags: "rxd"
    callers() && raise(E_PERM);
    $login.checkpoint_in_progress = 1;
    `$local.checkpoint_notification:checkpoint_started(@args) ! ANY';
  endverb

  verb bf_force_input (this none this) owner: #2 flags: "rxd"
    "Copied from Jay (#3920):bf_force_input Mon Jun 16 20:55:27 1997 PDT";
    "force_input(conn, line [, at-front])";
    "see help on the builtin for more information. This verb is called by the server when $server_options.protect_force_input exists and is true and caller_perms() are not wizardly.";
    {conn, line, ?at_front = 0} = args;
    if (caller_perms() != conn)
      retval = E_PERM;
    elseif (conn in $login.newted)
      retval = E_PERM;
    else
      retval = `force_input(@args) ! ANY';
      this.force_input_count = this.force_input_count + 1;
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb moveto (this none this) owner: #2 flags: "rxd"
    "Let's keep bozos from partying.  --Nosredna the partypooper";
    return pass(#-1);
  endverb

  verb "bf_set_property_info set_property_info" (this none this) owner: #2 flags: "rxd"
    who = caller_perms();
    retval = 0;
    try
      {what, propname, info} = args;
    except (E_ARGS)
      retval = E_ARGS;
    endtry
    try
      {owner, perms, ?newname = 0} = info;
    except (E_ARGS)
      retval = E_ARGS;
    except (E_TYPE)
      retval = E_TYPE;
    endtry
    if (retval != 0)
    elseif (newname in {"object_size", "size_quota", "queued_task_limit"} && !who.wizard)
      retval = E_PERM;
    else
      set_task_perms(who);
      retval = `set_property_info(@args) ! ANY';
    endif
    return typeof(retval) == ERR && $code_utils:dflag_on() ? raise(retval) | retval;
  endverb

  verb include_for_core (this none this) owner: #2 flags: "rxd"
    return properties(this);
  endverb

  verb handle_task_timeout (this none this) owner: #2 flags: "rxd"
    if (!callers())
      {resource, stack, traceback} = args;
      if (!$object_utils:connected(player))
        "Mail the player the traceback if e isn't connected.";
        $mail_agent:send_message(#0, player, {"traceback", $gripe_recipients}, traceback);
      endif
      "now let the player do something with it if e wants...";
      return `player:(verb)(@args) ! ANY';
    endif
  endverb
endobject