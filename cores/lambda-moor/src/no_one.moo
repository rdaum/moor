object NO_ONE
  name: "Everyman"
  parent: MAIL_RECIPIENT_CLASS
  owner: HACKER
  player: true
  programmer: true
  readable: true

  property queued_task_limit (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = 0;

  override aliases (owner: BYTE_QUOTA_UTILS_WORKING, flags: "r") = {"Everyman", "everyone", "no_one", "noone"};
  override description = "The character used for \"safe\" evals.";
  override home = LOCAL;
  override last_disconnect_time = 2147483647;
  override mail_forward = "Everyman ($no_one) can not receive mail.";
  override object_size = {5625, 1084848672};
  override ownership_quota = -10000;
  override page_echo_msg = "... no one out there to see it.";
  override size_quota = {0, 0, 1084781037, 0};

  verb eval (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "eval(code)";
    "Evaluate code with $no_one's permissions (so you won't damage anything).";
    "If code does not begin with a semicolon, set this = caller (in the code to be evaluated) and return the value of the first `line' of code.  This means that subsequent lines will not be evaluated at all.";
    "If code begins with a semicolon, set this = caller and let the code decide for itself when to return a value.  This is how to do multi-line evals.";
    exp = args[1];
    if (this:bad_eval(exp))
      return E_PERM;
    endif
    set_task_perms(this);
    if (exp[1] != ";")
      return eval(tostr("this=", caller, "; return ", exp, ";"));
    else
      return eval(tostr("this=", caller, ";", exp, ";"));
    endif
  endverb

  verb moveto (this none this) owner: HACKER flags: "rxd"
    return 0;
  endverb

  verb eval_d (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":eval_d(code)";
    "exactly like :eval except that the d flag is unset";
    "Evaluate code with $no_one's permissions (so you won't damage anything).";
    "If code does not begin with a semicolon, set this = caller (in the code to be evaluated) and return the value of the first `line' of code.  This means that subsequent lines will not be evaluated at all.";
    "If code begins with a semicolon, set this = caller and let the code decide for itself when to return a value.  This is how to do multi-line evals.";
    exp = args[1];
    if (this:bad_eval(exp))
      return E_PERM;
    endif
    set_task_perms(this);
    if (exp[1] != ";")
      return $code_utils:eval_d(tostr("this=", caller, "; return ", exp, ";"));
    else
      return $code_utils:eval_d(tostr("this=", caller, ";", exp, ";"));
    endif
  endverb

  verb call_verb (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    "call_verb(object, verb name, args)";
    "Call verb with $no_one's permissions (so you won't damage anything).";
    "One could do this with $no_one:eval, but ick.";
    set_task_perms(this);
    return (args[1]):((args[2]))(@args[3]);
  endverb

  verb bad_eval (this none this) owner: BYTE_QUOTA_UTILS_WORKING flags: "rxd"
    ":bad_eval(exp)";
    "  Returns 1 if the `exp' is inappropriate for use by $no_one.  In particular, if `exp' contains calls to `eval', `fork', `suspend', or `call_function' it is bad.  Similarly, if `player' is a nonvalid object (or a child of $garbage) the expression is considered `bad' because it is likely an attempt to anonymously spoof.";
    "  At present, the checks for bad builtins are overzealous.  It should check for delimited uses of the above calls, in case someone has a variable called `prevalent'.";
    {exp} = args;
    if (index(exp, "eval") || index(exp, "fork") || index(exp, "suspend") || index(exp, "call_function"))
      "Well, they had one of the evil words in here.  See if it was in a quoted string or not -- we want to permit player:tell(\"Gentlemen use forks.\")";
      for bad in ({"eval", "fork", "suspend", "call_function"})
        tempindex = 1;
        while (l = index(exp[tempindex..$], bad, 0))
          if ($code_utils:inside_quotes(exp[1..tempindex + l - 1]))
            tempindex = tempindex + l;
          else
            "it's there, bad unquoted string";
            return 1;
          endif
        endwhile
      endfor
    endif
    if (!$recycler:valid(player) && player >= #0)
      return 1;
    endif
    return 0;
  endverb

  verb "set_*" (this none this) owner: HACKER flags: "rxd"
    if (!caller_perms().wizard)
      return E_PERM;
    else
      return pass(@args);
    endif
  endverb
endobject