object MAIL_OPTIONS
  name: "Mail Options"
  parent: GENERIC_OPTIONS
  owner: HACKER
  readable: true

  property choices_rn_order (owner: HACKER, flags: "rc") = {
    {"read", {".current_message folders are sorted by last read date."}},
    {"send", {".current_message folders are sorted by last send date."}},
    {"fixed", {".current_message folders are not sorted."}}
  };
  property show_all (owner: HACKER, flags: "rc") = {
    "Replies will go to original sender only.",
    "Replies will go to original sender and all previous recipients."
  };
  property show_enter (owner: HACKER, flags: "rc") = {
    "Mail editor will not start with an implicit `enter' command.",
    "Mail editor will start with an implicit `enter' command."
  };
  property show_expert (owner: HACKER, flags: "rc") = {"Novice mail user...", "Expert mail user..."};
  property show_expert_netfwd (owner: HACKER, flags: "rc") = {
    "@netforward confirms before emailing messages",
    "@netforward doesn't confirm before emailing messages"
  };
  property show_followup (owner: HACKER, flags: "rc") = {
    "No special reply action for messages with non-player recipients.",
    "Replies go only to first non-player recipient if any."
  };
  property show_include (owner: HACKER, flags: "rc") = {
    "Original message will not be included in replies",
    "Original message will be included in replies"
  };
  property show_no_auto_forward (owner: HACKER, flags: "rc") = {
    "@netforward when expiring messages",
    "do not @netforward messages when expiring mail"
  };
  property show_no_dupcc (owner: HACKER, flags: "r") = {
    "i want to read mail to me also sent to lists i read",
    "don't send me personal copies of mail also sent to lists i read"
  };
  property show_no_unsend (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {
    "People may @unsend unread messages they send to me",
    "No one may @unsend messages they sent to me"
  };
  property show_nosubject (owner: HACKER, flags: "rc") = {
    "Mail editor will initially require a subject line.",
    "Mail editor will not initially require a subject line."
  };
  property show_resend_forw (owner: HACKER, flags: "rc") = {
    "@resend puts player in Resent-By: header",
    "@resend puts player in From: header (like @forward)"
  };
  property "type_@mail" (owner: HACKER, flags: "rc") = {2, {2}};
  property "type_@unsend" (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {2, {2}};
  property type_expire (owner: HACKER, flags: "rc") = {0};
  property type_manymsgs (owner: HACKER, flags: "rc") = {0};
  property type_replyto (owner: HACKER, flags: "rc") = {1, {1}};
  property unsend_sequences (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {"before", "after", "since", "until", "subject", "body", "last"};

  override _namelist = "!include!noinclude!all!sender!nosubject!expert!enter!sticky!@mail!manymsgs!replyto!netmail!expire!followup!resend_forw!rn_order!no_auto_forward!expert_netfwd!news!no_dupcc!no_unsend!@unsend!";
  override aliases = {"Mail Options"};
  override description = "Options for mailing";
  override extras = {"noinclude", "sender"};
  override names = {
    "include",
    "all",
    "followup",
    "nosubject",
    "expert",
    "enter",
    "sticky",
    "@mail",
    "manymsgs",
    "replyto",
    "netmail",
    "expire",
    "resend_forw",
    "rn_order",
    "no_auto_forward",
    "expert_netfwd",
    "news",
    "no_dupcc",
    "no_unsend",
    "@unsend"
  };
  override namewidth = 19;
  override object_size = {14349, 1084848672};

  verb actual (this none this) owner: HACKER flags: "rxd"
    if (i = args[1] in {"noinclude", "sender"})
      return {{{"include", "all"}[i], !args[2]}};
    else
      return {args};
    endif
  endverb

  verb "parse_@mail" (this none this) owner: HACKER flags: "rxd"
    "... we'll take anything...";
    raw = args[2];
    if (raw == 1)
      "...+@mail => @mailo=new";
      return {args[1], "new"};
    else
      return args[1..2];
    endif
  endverb

  verb "parse_sticky parse_manymsgs" (this none this) owner: HACKER flags: "rxd"
    {oname, raw, data} = args;
    if (typeof(raw) == LIST)
      if (length(raw) > 1)
        return "Too many arguments.";
      endif
      raw = raw[1];
    elseif (typeof(raw) == INT)
      return {oname, raw && (oname == "manymsgs" ? 20 | 1)};
    endif
    if ((value = $code_utils:toint(raw)) == E_TYPE)
      return tostr("`", raw, "'?  Number expected.");
    endif
    return {oname, value};
  endverb

  verb parse_replyto (this none this) owner: HACKER flags: "rxd"
    {oname, raw, data} = args;
    if (typeof(raw) == STR)
      raw = $string_utils:explode(raw, ",");
    elseif (typeof(raw) == INT)
      return raw ? "You need to give one or more recipients." | {oname, 0};
    endif
    value = $mail_editor:parse_recipients({}, raw);
    if (value)
      return {oname, value};
    else
      return "No valid recipients in list.";
    endif
  endverb

  verb show_manymsgs (this none this) owner: HACKER flags: "rxd"
    value = this:get(@args);
    if (value)
      return {tostr(value), {tostr("Query when asking for ", value, " or more messages.")}};
    else
      return {0, {"Willing to be spammed with arbitrarily many messages/headers"}};
    endif
  endverb

  verb show_sticky (this none this) owner: HACKER flags: "rxd"
    value = this:get(@args);
    if (value)
      return {value, {"Sticky folders:  mail commands default to whatever", "mail collection the previous successful command looked at."}};
    else
      return {0, {"Teflon folders:  mail commands always default to `on me'."}};
    endif
  endverb

  verb "show_@mail" (this none this) owner: HACKER flags: "rxd"
    if (value = this:get(@args))
      return {"", {tostr("Default message sequence for @mail:  ", typeof(value) == STR ? value | $string_utils:from_list(value, " "))}};
    else
      default = $mail_agent.("player_default_@mail");
      return {0, {tostr("Default message sequence for @mail:  ", typeof(default) == STR ? default | $string_utils:from_list(default, " "))}};
    endif
  endverb

  verb show_replyto (this none this) owner: HACKER flags: "rxd"
    if (value = this:get(@args))
      return {"", {tostr("Default Reply-to:  ", $mail_agent:name_list(@value))}};
    else
      return {0, {"No default Reply-to: field"}};
    endif
  endverb

  verb show (this none this) owner: HACKER flags: "rxd"
    if (o = (name = args[2]) in {"sender", "noinclude"})
      args[2] = {"all", "include"}[o];
      return {@pass(@args), tostr("(", name, " is a synonym for -", args[2], ")")};
    else
      return pass(@args);
    endif
  endverb

  verb check_replyto (this none this) owner: HACKER flags: "rxd"
    "... must be object, list of objects, or false...";
    value = args[1];
    if (typeof(value) == OBJ)
      return {{value}};
    elseif (!this:istype(value, {{OBJ}}))
      return $string_utils:capitalize("Object or list of objects expected.");
    else
      return {value};
    endif
  endverb

  verb show_netmail (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (value = this:get(@args))
      return {value, {"Have MOO-mail automatically forwarded to me at", "my registered email-address."}};
    else
      return {value, {"Receive MOO-mail here on the MOO."}};
    endif
    "Last modified Tue Jun  1 02:10:08 1993 EDT by Edison@OpalMOO (#200).";
  endverb

  verb check_netmail (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":check_netmail(value) => Makes sure the email-address is one that can actually be used by $network:sendmail().";
    "The actual value sent is not checked since it can only be a boolean flag.  The player's email_address property is what is checked.";
    "Possible situations where the address would be unusable are when the address is invalid or we can't connect to the site to send mail.";
    "Returns a string error message if unusable or {value} otherwise.";
    if (caller != this)
      return E_PERM;
    endif
    if (args[1] && (reason = $network:email_will_fail($wiz_utils:get_email_address(player))))
      return tostr("Invalid registered email_address: ", reason);
    endif
    return args;
  endverb

  verb show_expire (this none this) owner: HACKER flags: "rxd"
    value = this:get(args[1], "expire");
    if (value < 0)
      return {1, {"Messages will not expire."}};
    else
      return {value, {tostr("Unkept messages expire in ", $time_utils:english_time(value || $mail_agent.player_expire_time), value ? "" | " (default)")}};
    endif
  endverb

  verb parse_expire (this none this) owner: HACKER flags: "rxd"
    {oname, value, data} = args;
    if (typeof(value) == STR && index(value, " "))
      value = $string_utils:explode(value, " ");
      if (!value)
        return {oname, 0};
      endif
    endif
    if (value == 1)
      return {oname, -1};
    elseif (typeof(value) == LIST)
      if (length(value) > 1)
        nval = $time_utils:parse_english_time_interval(@value);
        if (typeof(nval) == ERR)
          return "Time interval should be of a form like \"30 days, 10 hours and 43 minutes\".";
        else
          return {oname, nval};
        endif
      endif
      value = value[1];
    endif
    if ((nval = $code_utils:toint(value)) || nval == 0)
      return {oname, nval < 0 ? -1 | nval};
    elseif (value == "Never")
      return {oname, -1};
    else
      return "Number, time interval (e.g., \"30 days\"), or \"Never\" expected";
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rx"
    if (caller_perms().wizard)
      for x in ({"fast_check", "idle_check", "idle_threshold"})
        this:remove_name(x);
        for y in ({"show", "check", "parse"})
          delete_verb(this, y + "_" + x);
          delete_property(this, y + "_" + x);
        endfor
      endfor
      pass(@args);
    endif
  endverb

  verb check_news (this none this) owner: HACKER flags: "rxd"
    if ((what = args[1]) in {"new", "contents", "all"})
      return {what};
    else
      return "Error: `news' option must be one of `new' or `contents' or `all'";
    endif
  endverb

  verb parse_news (this none this) owner: HACKER flags: "rxd"
    if (typeof(args[2]) == INT)
      return tostr(strsub(verb, "parse_", ""), " is not a boolean option.");
    else
      return {args[1], typeof(args[2]) == STR ? args[2] | $string_utils:from_list(args[2], " ")};
    endif
  endverb

  verb show_news (this none this) owner: HACKER flags: "rxd"
    if ((value = this:get(@args)) == "all")
      return {value, {"the `news' command will show all news"}};
    elseif (value == "contents")
      return {value, {"the `news' command will show the titles of all articles"}};
    elseif (value == "new")
      return {value, {"the `news' command will show only new news"}};
    else
      return {0, {"the `news' command will show all news"}};
    endif
  endverb

  verb "parse_@unsend" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {name, value, bleh} = args;
    if (typeof(value) == INT)
      return tostr(name, " is not a boolean option.");
    elseif (typeof(value) == STR)
      value = {value};
    endif
    ok = this.unsend_sequences;
    for x in (value)
      if (!(pos = index(x, ":")) || !(x[1..pos - 1] in ok))
        return tostr("Invalid sequence - ", x);
      elseif (pos != rindex(x, ":"))
        return tostr("As a preventative measure, you may not use more than one : in a sequence. The following sequence is therefore invalid - ", x);
      endif
    endfor
    return {name, value};
  endverb

  verb "show_@unsend" (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (value = this:get(@args))
      return {"", {tostr("Default message sequence for @unsend:  ", typeof(value) == STR ? value | $string_utils:from_list(value, " "))}};
    else
      default = $mail_agent.("player_default_@unsend");
      return {0, {tostr("Default message sequence for @unsend:  ", typeof(default) == STR ? default | $string_utils:from_list(default, " "))}};
    endif
  endverb
endobject