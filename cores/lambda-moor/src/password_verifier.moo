object PASSWORD_VERIFIER
  name: "password verifier"
  parent: THING
  owner: BYTE_QUOTA_UTILS_WORKING
  readable: true

  property check_against_dictionary (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property check_against_email (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property check_against_hosts (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property check_against_moo (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property check_against_name (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property check_obscure_stuff (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property help_msg (owner: HACKER, flags: "r") = {
    "Password Verifier",
    "==================",
    "",
    "To check for the validity of a password, use",
    "  :reject_password( password [, for-whom? ] )",
    "... If it returns a true value, that value will contain the string representing the reason why the password was rejected.  If it returns a false value, the password is OK.",
    "",
    "The toggle switches for this checking are:"
  };
  property minimum_password_length (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;
  property require_funky_characters (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;

  override aliases = {"password verifier", "password", "verifier", "pwd"};
  override description = "The password verifier verifies passwords.";
  override object_size = {10921, 1084848672};

  verb help_msg (this none this) owner: HACKER flags: "rxd"
    if (typeof(base = this.(verb)) == STR)
      base = {base};
    endif
    base = {@base, "", tostr(".minimum_password_length = ", toliteral(x = this.minimum_password_length)), x ? tostr("Passwords are required to be a minimum of ", $string_utils:english_number(x), " characters in length.") | "There is no minimum length requirement for passwords."};
    base = {@base, "", tostr(".check_against_moo = ", toliteral(x = this.check_against_moo)), tostr("Passwords ", x ? "may not" | "may", " be variants on the MOO's name (", $network.MOO_name, ").")};
    base = {@base, "", tostr(".check_against_name = ", toliteral(x = this.check_against_name)), tostr("Passwords ", x ? "may not" | "may", " be variants on the player's MOO name and/or aliases.")};
    base = {@base, "", tostr(".check_against_email = ", toliteral(x = this.check_against_email)), x ? "Passwords may not be variants on the player's email address." | "Passwords are not checked against the player's email address."};
    base = {@base, "", tostr(".check_against_hosts = ", toliteral(x = this.check_against_hosts)), x ? "Passwords may not be variants on the player's hostname(s)." | "Passwords are not checked against the player's hostname(s)."};
    base = {@base, "", tostr(".check_against_dictionary = ", toliteral(x = this.check_against_dictionary)), tostr("Passwords ", typeof(x) in {LIST, OBJ} ? "may not" | "may", " be dictionary words.", x && !$network.active ? "  (This option is set but unavailable.)" | "")};
    base = {@base, "", tostr(".require_funky_characters = ", toliteral(x = this.require_funky_characters)), tostr("Non-alphabetic characters are ", x ? "" | "not ", "required in passwords.")};
    base = {@base, "", tostr(".check_obscure_stuff = ", toliteral(x = this.check_obscure_stuff)), x ? "Misc. obscure checks enabled" | "No obscure checks in use."};
    return base;
  endverb

  verb reject_password (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":reject_password ( STR password [ , OBJ for-whom ] );";
    "=> string value [if the password is rejected, why?]";
    "=> false value [if the password isn't rejected]";
    if (length(args) == 1)
      trust = 0;
    else
      if ($perm_utils:controls(caller_perms(), args[2]))
        trust = 1;
      else
        return "Permissions don't permit setting of that password.";
      endif
    endif
    "this is gonna be huge";
    return this:trivial_check(@args) || (this.minimum_password_length && this:check_length(@args)) || (this.check_against_name && trust && this:check_name(@args)) || (this.check_against_email && trust && this:check_email(@args)) || (this.check_against_hosts && trust && this:check_hosts(@args)) || (typeof(this.check_against_dictionary) in {LIST, OBJ} && this:check_dictionary(@args)) || (this.require_funky_characters && this:check_for_funky_characters(@args)) || (this.check_against_moo && this:check_against_moo(@args)) || (this.check_obscure_stuff && this:check_obscure_combinations(@args));
  endverb

  verb trivial_check (this none this) owner: HACKER flags: "rxd"
    if (typeof(pwd = args[1]) != STR)
      return "Passwords must be strings.";
    elseif (index(pwd, " "))
      return "Passwords may not contain spaces.";
    elseif (length(args) == 2)
      if (typeof(who = args[2]) != OBJ || !valid(who) || !is_player(who))
        return "That's not a player.";
      elseif (!$perm_utils:controls(caller_perms(), who))
        return "You can't set the password for that player.";
      elseif ($object_utils:isa(who, $guest))
        return "Sorry, but guest characters are not allowed to change their passwords.";
      endif
    endif
  endverb

  verb check_length (this none this) owner: HACKER flags: "rxd"
    if ((l = this.minimum_password_length) && length(args[1]) < l)
      return tostr("Passwords must be a minimum of ", $string_utils:english_number(l), l == 1 ? " character " | " characters ", "long.");
    endif
  endverb

  verb check_name (this none this) owner: HACKER flags: "rxd"
    pwd = args[1];
    if (valid($player_db:find_exact(pwd)))
      return "Passwords may not be close to a player's name/alias pair.";
    elseif (valid($player_db:find($string_utils:reverse(pwd))))
      return "Passwords ought not be the reverse of a player's name/alias.";
    endif
  endverb

  verb check_email (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {pwd, who} = args;
    if (!$perm_utils:controls(caller_perms(), who))
      return "Permission denied.";
    endif
    email = $wiz_utils:get_email_address(who);
    if (!email)
      "can't check";
      return;
    endif
    if (index(email, pwd))
      return "Passwords can't match your registered email address.";
    endif
  endverb

  verb check_hosts (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    {pwd, who} = args;
    if (!$perm_utils:controls(caller_perms(), who))
      return "Permission denied.";
    endif
    hosts = who.all_connect_places;
    for x in (hosts)
      if (index(x, pwd))
        return "Passwords may not match hostnames.";
      endif
    endfor
  endverb

  verb check_dictionary (this none this) owner: HACKER flags: "rxd"
    pwd = args[1];
    if (typeof(dict = this.check_against_dictionary) == LIST && $network.active)
      "assume we're checking an on-line dictionary";
      dict[3] = dict[3] + pwd;
      result = $gopher:get(@dict);
      if (typeof(result) == ERR)
        "we probably can't check the dictionary anyway";
        return;
      elseif (result[1] && result[1][1] != "0" && !this:_is_funky_case(pwd))
        return "Dictionary words are not permitted for passwords.";
      endif
    elseif (typeof(dict) == OBJ)
      "assume we're checking mr spell";
      try
        if (dict:find_exact(pwd) && !this:_is_funky_case(pwd))
          return "Dictionary words are not permitted for passwords.";
        endif
      except (ANY)
        "in case this is messed up. Just let it go and return 0;";
      endtry
    endif
  endverb

  verb check_for_funky_characters (this none this) owner: HACKER flags: "rxd"
    if (this:_is_funky_case(pwd = args[1]))
      return;
    endif
    alphabet = $string_utils.alphabet;
    for i in [1..length(pwd)]
      if (!index(alphabet, pwd[i]))
        return;
      endif
    endfor
    return "At least one unusual capitalization and/or numeric or punctuation character is required.";
  endverb

  verb check_against_moo (this none this) owner: HACKER flags: "rxd"
    pwd = args[1];
    moo = $network.MOO_Name;
    if (this:_is_funky_case(pwd))
      return;
    endif
    if (pwd == moo)
      return "The MOO's name is not secure as a password.";
    endif
    if (moo[$ - 2..$] == "MOO")
      if (pwd == moo[1..$ - 3])
        return "The MOO's name is not secure as a password.";
      endif
    endif
  endverb

  verb _is_funky_case (this none this) owner: HACKER flags: "rxd"
    pwd = args[1];
    if (!strcmp(pwd, u = $string_utils:uppercase(pwd)))
      return 0;
    elseif (!strcmp(pwd, l = $string_utils:lowercase(pwd)))
      return 0;
    elseif (!strcmp(pwd, tostr(u[1], l[2..$])))
      return 0;
    else
      return 1;
    endif
  endverb

  verb check_obscure_combinations (this none this) owner: HACKER flags: "rxd"
    pwd = args[1];
    if (match(pwd, "^[0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9][0-9][0-9]$"))
      return "Social security numbers are potentially insecure passwords.";
    elseif (match(pwd, "^[0-9]+/[0-9]+/[0-9]+$"))
      return "Passwords which look like dates are potentially insecure passwords.";
    endif
  endverb

  verb init_for_core (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    if (caller_perms().wizard)
      pass(@args);
      this.minimum_password_length = this.check_against_name = this.check_against_email = this.check_against_hosts = this.check_against_dictionary = this.require_funky_characters = this.check_against_moo = this.check_obscure_stuff = 0;
    endif
  endverb
endobject