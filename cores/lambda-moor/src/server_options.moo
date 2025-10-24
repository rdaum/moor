object SERVER_OPTIONS
  name: "Server Options"
  parent: ROOT_CLASS
  owner: #2
  readable: true

  property bg_ticks (owner: #2, flags: "rc") = 80000;
  property boot_msg (owner: #2, flags: "rc") = "";
  property connect_msg (owner: #2, flags: "rc") = "*** Connected ***";
  property fg_ticks (owner: #2, flags: "rc") = 150000;
  property help_msg (owner: #2, flags: "rc") = {
    "                Server Options <$server_options>",
    "                --------------------------------",
    "",
    "messages: 'boot_msg', 'connect_msg', 'create_msg', 'recycle_msg', 'redirect_from_msg', 'redirect_to_msg', and 'timeout_msg'.",
    "A number of the messages printed to a connection by the server under various circumstances can now be customized or eliminated from within the DB.  In each case, a property on $server_options is checked at the time the message would be printed.  If the property does not exist, the standard message is printed.  If the property exists and its value is not a string, then no message is printed at all.  Otherwise, the string is printed in place of the standard message.  The following list covers all of the newly customizable messages, showing for each the name of the relevant property on $server_options, the default/standard message, and the circumstances under which the message is printed:",
    "'boot_msg'              \"*** Disconnected ***\"",
    "The function boot_player() was called on this connection.",
    "'connect_msg'           \"*** Connected ***\"",
    "The user object that just logged in on this connection existed before #0:do_login_command() was called.",
    "'create_msg'            \"*** Created ***\"",
    "The user object that just logged in on this connection did not exist before #0:do_login_command() was called.",
    "'recycle_msg'           \"*** Recycled ***\"",
    "The logged-in user of this connection has been recycled.",
    "'redirect_from_msg'     \"*** Redirecting connection to new port ***\"",
    "The logged-in user of this connection has just logged in on some other connection.",
    "'redirect_to_msg'       \"*** Redirecting old connection to this port ***\"",
    "The user who just logged in on this connection was already logged in on some other connection.",
    "'timeout_msg'           \"*** Timed-out waiting for login. ***\"",
    "This in-bound network connection was idle and un-logged-in for at least CONNECT_TIMEOUT seconds (as defined in options.h).",
    "",
    "Note: on a 1.8rN server, changes to $server_options will not take effect until load_server_options() has been called.",
    "",
    "",
    "Some properties on $server_options can change the server behavior:",
    "",
    "'bg_seconds', 'bg_ticks', 'fg_seconds', and 'fg_ticks'.",
    "If those properties exist and are numbers, the server use them instead of the constants DEFAULT_BG_SECONDS, DEFAULT_BG_TICKS, DEFAULT_FG_SECONDS and DEFAULT_FG_TICKS (respectively) defined at compile time in \"options.h\"; they are looked up anew every time a task begins or resumes execution. Those define ticks (basic operations)/real-time seconds any task is allowed to use without suspending. 'fg' constants/properties are used only for 'foreground' tasks (those started by either player input or the server's initiative and that have never suspended); the 'bg' constants/properties are used only for 'background' tasks (forked tasks and those of any kind that have suspended).",
    "",
    "'max_stack_depth' This allow to change in-db the the maximum verb-call depth. Originillay the maximum verb-call depth is defined at compile time by the DEFAULT_MAX_STACK_DEPTH constant in \"options.h\". The maximum stack depth for any task is set at the time that task is created and cannot be changed thereafter. This implies that suspended tasks, even after being saved in and restored from the DB, are not affected by later changes to $server_options.max_stack_depth. ",
    "",
    "'queued_task_limit' if this property exist and its value is non-negative, then it is used as the maximum of tasks a verb-owner (more exactly the user's perms the verb run with) can queue (through fork() and suspend()). This setting is overriden if the user has a 'queued_task_limit' property and if its value is non-negative. E_QUOTA is raised of either forking or suspending when the user is over quota for tasks.",
    "",
    "'protect_...' On every call to a built-in function 'foo', if the property $server_options.protect_foo exists and is true, and the programmer is not a wizard, then the server checks for the existence of #0:bf_<fuction> and calls that. If it doesn't exist then E_PERM is raised, i.e. the built-in function is made wiz-only.",
    "                --------------------------------"
  };
  property permit_writable_verbs (owner: #2, flags: "rc") = 0;
  property protect_add_property (owner: #2, flags: "rc") = 1;
  property protect_add_verb (owner: #2, flags: "rc") = 1;
  property protect_chparent (owner: #2, flags: "rc") = 1;
  property protect_force_input (owner: #2, flags: "rc") = 1;
  property protect_recycle (owner: #2, flags: "rc") = 1;
  property protect_set_property_info (owner: #2, flags: "r") = 1;
  property protect_set_verb_info (owner: #2, flags: "rc") = 1;
  property queued_task_limit (owner: #2, flags: "rc") = 300;
  property support_numeric_verbname_strings (owner: HACKER, flags: "r") = 0;

  override aliases = {"Server Options"};
  override object_size = {6853, 1084848672};

  verb help_msg (this none this) owner: HACKER flags: "rxd"
    output = {"On $server_options, the following settings have been established by the wizards:", ""};
    wizonly = {};
    etc = {};
    mentioned = {};
    for x in (setremove(properties(this), "help_msg"))
      if (index(x, "protect_") == 1)
        mentioned = {@mentioned, x[9..$]};
        wizonly = {@wizonly, tostr(x[9..$], "() is ", this.(x) ? "" | "not ", "wizonly.")};
      else
        etc = {@etc, tostr("$server_options.", x, " = ", $string_utils:print(this.(x)))};
      endif
    endfor
    if ("set_verb_code" in wizonly)
      wizonly = {@wizonly, "", "Note: since the 'set_verb_code' built-in function is wiz-only, then the '.program' built-in command is wiz-only too."};
    endif
    if (bf = $set_utils:intersection(verbs(#0), mentioned))
      bf = $list_utils:sort(bf);
      etc = {@etc, "", "In your code, #0:(built-in)(@args) should be called rather than built-in(@args) when you would use one of the following built-in functions:", $string_utils:english_list(bf) + ".", "Example: #0:" + bf[1] + "(@args) should be used instead of " + bf[1] + "(@args)"};
    endif
    return {@this.help_msg, @output, @wizonly, "", @etc};
  endverb

  verb init_for_core (this none this) owner: #2 flags: "rxd"
    if (!caller_perms().wizard)
      raise(E_PERM);
    endif
    this.support_numeric_verbname_strings = 0;
    pass(@args);
  endverb
endobject