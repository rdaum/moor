object LOGIN [
  import_export_id -> "login",
  import_export_hierarchy -> {"auth"}
]
  name: "Login Service"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property blank_command (owner: ARCH_WIZARD, flags: "r") = "welcome";
  property bogus_command (owner: ARCH_WIZARD, flags: "r") = "?";
  property connection_quiet_period (owner: ARCH_WIZARD, flags: "rc") = 7200;
  property default_home (owner: ARCH_WIZARD, flags: "rc") = FIRST_ROOM;
  property default_player_class (owner: ARCH_WIZARD, flags: "r") = PLAYER;
  property intercepted_actions (owner: ARCH_WIZARD, flags: "rc") = {};
  property intercepted_players (owner: ARCH_WIZARD, flags: "rc") = {};
  property moo_title (owner: ARCH_WIZARD, flags: "rc") = "Cowbell-Core";
  property new_player_arrival_template (owner: ARCH_WIZARD, flags: "rc") = "{nc} {have|has} just arrived.";
  property new_player_letter (owner: ARCH_WIZARD, flags: "rc") = {
    ARCH_WIZARD,
    "Welcome to Cowbell!",
    {
      "Hello and welcome!",
      "",
      "We're glad you've joined us. Feel free to explore, meet other players, and make yourself at home.",
      "",
      "If you need help, try typing `help` or `what` to see what you can do.",
      "",
      "Enjoy your stay!"
    }
  };
  property new_player_welcome_message (owner: ARCH_WIZARD, flags: "rc") = {
    "Welcome to {TITLE}!",
    "",
    "Try entering `help` to see what kind of things you can do where you are."
  };
  property oauth2_identity_version (owner: ARCH_WIZARD, flags: "") = 0;
  property player_creation_enabled (owner: ARCH_WIZARD, flags: "r") = true;
  property player_setup_capability (owner: LOGIN, flags: "") = <PLAYER, .token = "v4.local.EIjSChEcQf8hjLCih4NGE-vKw_UZDTKRpWaYiZeQP615jQATzm-KoZTU_t7DfF8lVdOkzNqSRrItjVEZczaN6BIB-83GPs-xGAM4eg9J8sb3NJJr8z8sJPXh2uNurXg4vEbB5TMhj04AQsuski87Jmwe0r1kEq1cS5baIer5griqGFykpZBCHuieE382dS8XJdOzq0p9xViQ9-x_87dmbVdJPAP0tbxA-7KycBk72eldC-mGBTPjfD2qQWqhczzmB77RJ1azUhhOTZU4g6uEBEBfLgE8a-heeB_AIqK1zKl_t8lOf-vUq9rUEQChG5YJID6_NNZGNB8y68eciVHUD1lPnPOaeCc">;
  property player_wakeup_template (owner: ARCH_WIZARD, flags: "rc") = "{nc} {have|has} woken up.";
  property post_creation_setup_enabled (owner: ARCH_WIZARD, flags: "rc") = true;
  property post_creation_setup_fields (owner: ARCH_WIZARD, flags: "rc") = "pronouns,description,picture";
  property post_creation_setup_title (owner: ARCH_WIZARD, flags: "rc") = "Set Up Your Profile";
  property privacy_policy (owner: ARCH_WIZARD, flags: "rc") = {
    "## Privacy Policy",
    "",
    "### What We Collect",
    "",
    "When you create an account, we store:",
    "",
    "- Your chosen player name",
    "- Your password (encrypted)",
    "- Your session history (encrypted with your encryption password)",
    "",
    "### How We Use Your Data",
    "",
    "Your data is used solely to provide you with access to this server. We do not sell or share your information with third parties.",
    "",
    "### Session History",
    "",
    "Your session history is end-to-end encrypted. Only you can decrypt it using your encryption password. Server administrators cannot read your history.",
    "",
    "### Data Retention",
    "",
    "Your account and history are retained as long as you maintain an active account. You may request deletion of your account and all associated data at any time.",
    "",
    "### Contact",
    "",
    "For privacy concerns, contact the server administrator."
  };
  property privacy_policy_content_type (owner: ARCH_WIZARD, flags: "rc") = "text/djot";
  property registration_string (owner: ARCH_WIZARD, flags: "rc") = "Character creation is disabled.";
  property welcome_message (owner: ARCH_WIZARD, flags: "rc") = {
    "## Welcome to the _mooR_ *Cowbell* core.",
    "",
    "connect with `archwizard` `test` to log in.",
    "",
    "Server version: {VERSION}",
    "Core version: {CORE_VERSION}",
    "",
    "You will probably want to change this text which is stored in $login.welcome_message property."
  };
  property welcome_message_kind (owner: ARCH_WIZARD, flags: "rc") = 'djot_blocks;

  override description (owner: ARCH_WIZARD, flags: "rc") = "Login service handling player authentication, character creation, and OAuth2 integration.";

  verb welcome (any none any) owner: ARCH_WIZARD flags: "rxd"
    "Show the authored welcome message through the public document delivery boundary.";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    this:_public_output(this:welcome_content());
  endverb

  method welcome_content owner: ARCH_WIZARD
    "Parse explicit banner syntax before inserting literal server facts into text nodes.";
    set_task_perms(caller_perms());
    const source = this.welcome_message:join("\n");
    const authored = $format:authored(this.welcome_message_kind, source);
    const bindings = ["TITLE" -> this.moo_title, "VERSION" -> server_version(),
      "CORE_VERSION" -> $sysobj.core_version];
    return $document_template:mk('blocks,
      {this:_welcome_node(block, 'block, bindings) for block in (flycontents(authored))});
  endmethod

  method _welcome_node owner: HACKER
    "Replace placeholders only in parsed text nodes; code, destinations, and labels stay authored.";
    const {value, expected, bindings} = args;
    const node = expected in {'inline, 'block};
    const kind = node ? value.kind | expected;
    if (kind == 'text)
      const text = value.text;
      let result = "";
      let cursor = 1;
      while (cursor <= length(text))
        let position = length(text) + 1;
        let token = "";
        for name in (mapkeys(bindings))
          const offset = index(text[cursor..$], "{" + name + "}", 1);
          if (offset && cursor + offset - 1 < position)
            position = cursor + offset - 1;
            token = name;
          endif
        endfor
        result = result + text[cursor..position - 1];
        if (!length(token))
          break;
        endif
        result = result + bindings[token];
        cursor = position + length(token) + 2;
      endwhile
      return $format.text:mk(result);
    endif
    kind in {'code_block, 'inline_code, 'link, 'reference} && return value;
    const shape = $document_schema:_shape(kind, "welcome");
    let fields = node ? flyslots(value) | value;
    node && (fields = mapdelete(fields, 'kind));
    const contents_key = shape['contents];
    if (typeof(contents_key) == TYPE_SYM)
      fields[contents_key] = flycontents(value);
    endif
    for child in (shape['children])
      const {key, child_kind, cardinality} = child;
      if (!maphaskey(fields, key))
        continue;
      endif
      let children = {};
      for entry in (fields[key])
        if (cardinality == 'matrix_optional)
          children = {@children, {this:_welcome_node(cell, child_kind, bindings) for cell in (entry)}};
        else
          children = {@children, this:_welcome_node(entry, child_kind, bindings)};
        endif
      endfor
      fields[key] = children;
    endfor
    !node && return fields;
    let contents = {};
    if (typeof(contents_key) == TYPE_SYM)
      contents = fields[contents_key];
      fields = mapdelete(fields, contents_key);
    endif
    return $document_template:mk(kind, contents, fields);
  endmethod

  method _public_output owner: ARCH_WIZARD
    "Send public login content to the current connection, or the HTTP output-capture session.";
    "No player object or history entry is created for unauthenticated output.";
    caller == this || raise(E_PERM);
    const {content} = args;
    const connections = connections();
    const target = length(connections) ? connections[1][1] | player;
    const format = length(connections) ? $format:connection_format(connections[1][4]) | 'text_djot;
    const event = $event:mk_login(false, content):with_audience('utility);
    const batch = event:render_for(false, {['format -> format]});
    for diagnostic in (batch['diagnostics])
      server_log("Login content: " + diagnostic);
    endfor
    const rendered = batch['results][1];
    const metadata = ['cowbell_document -> batch['document],
      'annotations -> rendered['annotations], 'delivery_id -> uuid(), 'audience -> 'utility];
    notify(target, rendered['body], false, false, tosym(strsub(tostr(format), "_", "/", 1)), metadata);
  endmethod

  verb "co*nnect @co*nnect" (any none any) owner: ARCH_WIZARD flags: "rxd"
    "$login:connect(player-name [, password])";
    " => 0 (for failed connections)";
    " => objnum (for successful connections)";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    "Check arguments, print usage notice if necessary";
    try
      {name, ?password = 0} = args;
      name = strsub(name, " ", "_");
    except (E_ARGS)
      this:_public_output(tostr("Usage:  ", verb, " <existing-player-name> <password>"));
      return 0;
    endtry
    try
      "Is our candidate name invalid?";
      if (!valid(candidate = (orig_candidate = this:_match_player(name))))
        raise(E_INVARG, tostr("`", name, "' matches no player name."));
      endif
      "We have a valid candidate, so we can now attempt to challenge it.";
      {status, _} = this:_password_state(candidate, password);
      if (status == 'ok)
        "Password verified.";
      elseif (status == 'missing)
        "We assume the password is a $password frob; prompt for it interactively.";
        set_connection_option(player, "binary", 1);
        this:_public_output("Password: ");
        set_connection_option(player, "binary", 0);
        set_connection_option(player, "client-echo", 0);
        this:add_interception(player, "intercepted_password", candidate);
        return 0;
      elseif (status == 'external_only)
        this:_server_log(tostr("FAILED CONNECT (NO PASSWORD): ", name, " (", candidate, ") on ", connection_name(player)));
        raise(E_INVARG, "This account uses external authentication.");
      elseif (status == 'invalid_type)
        this:_server_log(tostr("FAILED CONNECT (BAD PASSWORD TYPE): ", name, " (", candidate, ") on ", connection_name(player)));
        raise(E_INVARG, "Cannot authenticate this account.");
      else
        this:_server_log(tostr("FAILED CONNECT: ", name, " (", candidate, ") on ", connection_name(player)));
        raise(E_INVARG, "Invalid password.");
      endif
      "TODO: block lists, guests, etc";
      "Log the player in!";
      return candidate;
    except (E_INVARG)
      this:_public_output("Either that player does not exist, or has a different password.");
      return 0;
    endtry
  endverb

  verb oauth2_check (any none any) owner: ARCH_WIZARD flags: "rxd"
    "$login:oauth2_check(provider, external_id)";
    " => 0 (for not found)";
    " => objnum (for existing OAuth2 identity)";
    caller == #0 && callers()[1][2] == "do_oauth_login" || raise(E_PERM);
    try
      {provider, external_id} = args;
    except (E_ARGS)
      this:_public_output("OAuth2 check failed: invalid arguments");
      return 0;
    endtry
    candidate = this:find_by_oauth2(provider, external_id);
    this:_server_log(tostr("OAUTH2 CHECK: candidate=", candidate, " valid=", valid(candidate), " typeof=", typeof(candidate)));
    if (valid(candidate))
      this:_server_log(tostr("OAUTH2 CHECK SUCCESS: ", provider, ":", external_id, " -> ", candidate));
      return candidate;
    else
      this:_server_log(tostr("OAUTH2 CHECK NOT FOUND: ", provider, ":", external_id, " returning 0"));
      ret = 0;
      this:_server_log(tostr("OAUTH2 CHECK: about to return ", ret, " typeof=", typeof(ret)));
      return ret;
    endif
  endverb

  verb oauth2_create (any none any) owner: ARCH_WIZARD flags: "rxd"
    "$login:oauth2_create(provider, external_id, email, name, username, player_name)";
    " => 0 (for failed creation)";
    " => objnum (for successful creation)";
    caller == #0 && callers()[1][2] == "do_oauth_login" || raise(E_PERM);
    if (!this.player_creation_enabled)
      this:_public_output(this.registration_string);
      return 0;
    endif
    try
      {provider, external_id, email, name, username, player_name} = args;
      player_name = strsub(player_name, " ", "_");
    except (E_ARGS)
      this:_public_output("OAuth2 create failed: invalid arguments");
      return 0;
    endtry
    if (!player_name || player_name == "<>")
      this:_public_output("You can't have a blank name!");
      return 0;
    elseif (player_name[1] == "<" && player_name[$] == ">")
      this:_public_output("Don't use angle brackets in your player name.");
      return 0;
    elseif (index(player_name, " "))
      this:_public_output("Sorry, no spaces are allowed in player names.  Use dashes or underscores.");
      return 0;
    elseif (this:_match_player(player_name) != $failed_match)
      this:_public_output("Sorry, that name is not available.  Please choose another.");
      return 0;
    endif
    valid(this:find_by_oauth2(provider, external_id)) && return 0;
    new = this:_create_player(player_name, 0, email || "", {});
    this:_claim_oauth_identity(new, provider, external_id);
    this:_server_log(tostr("OAUTH2 CREATE: ", player_name, " (", new, ") via ", provider, ":", external_id));
    return new;
  endverb

  verb oauth2_connect (any none any) owner: ARCH_WIZARD flags: "rxd"
    "$login:oauth2_connect(provider, external_id, email, name, username, existing_name, existing_password)";
    " => 0 (for failed connection)";
    " => objnum (for successful link)";
    caller == #0 && callers()[1][2] == "do_oauth_login" || raise(E_PERM);
    try
      {provider, external_id, email, name, username, existing_name, existing_password} = args;
      existing_name = strsub(existing_name, " ", "_");
    except (E_ARGS)
      this:_public_output("OAuth2 connect failed: invalid arguments");
      return 0;
    endtry
    if (!valid(candidate = this:_match_player(existing_name)))
      this:_public_output("That player does not exist.");
      return 0;
    endif
    {status, _} = this:_password_state(candidate, existing_password);
    if (status == 'ok)
      "Password verified for linking.";
    elseif (status == 'external_only)
      this:_public_output("Set an account password before linking another login identity.");
      return 0;
    elseif (status == 'missing)
      this:_public_output("Invalid password for existing account.");
      return 0;
    elseif (status == 'invalid_type)
      this:_public_output("Cannot link to that account.");
      return 0;
    else
      this:_public_output("Invalid password for existing account.");
      return 0;
    endif
    this:_claim_oauth_identity(candidate, provider, external_id);
    if (email)
      try
        current_email = candidate.email_address;
      except (E_PROPNF)
        candidate.email_address = email;
        current_email = email;
      endtry
      if (typeof(current_email) != TYPE_STR || length(current_email) == 0)
        candidate.email_address = email;
      endif
    endif
    this:_server_log(tostr("OAUTH2 CONNECT: ", existing_name, " (", candidate, ") linked ", provider, ":", external_id));
    return candidate;
  endverb

  verb "cr*eate @cr*eate" (any none any) owner: ARCH_WIZARD flags: "rxd"
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    if (!this.player_creation_enabled)
      this:_public_output(this.registration_string);
      return;
    endif
    if (length(args) != 2)
      this:_public_output(tostr("Usage:  ", verb, " <new-player-name> <new-password>"));
      return;
    endif
    if (!(name = args[1]) || name == "<>")
      this:_public_output("You can't have a blank name!");
      if (name)
        this:_public_output("Also, don't use angle brackets (<>).");
      endif
      return;
    endif
    if (name[1] == "<" && name[$] == ">")
      this:_public_output("Try that again but without the angle brackets, e.g.,");
      this:_public_output(tostr(" ", verb, " ", name[2..$ - 1], " ", strsub(strsub(args[2], "<", ""), ">", "")));
      this:_public_output("This goes for other commands as well.");
      return;
    endif
    if (index(name, " "))
      this:_public_output("Sorry, no spaces are allowed in player names.  Use dashes or underscores.");
      "... lots of routines depend on there not being spaces in player names...";
      return;
    endif
    if (this:_match_player(name) != $failed_match)
      this:_public_output("Sorry, that name is not available.  Please choose another.");
      "... note the :_match_player call is not strictly necessary...";
      "... it is merely there to handle the case that $player_db gets corrupted.";
      return;
    endif
    if (!(password = args[2]))
      this:_public_output("You must set a password for your player.");
      return;
    endif
    return this:_create_player(name, password, "", {});
  endverb

  method _match_player owner: ARCH_WIZARD
    ":_match_player(name)";
    "This is the matching routine used by @connect.";
    "returns either a valid player corresponding to name or $failed_match.";
    caller == this || caller.wizard || raise(E_PERM);
    name = args[1];
    "Try to parse as object number first";
    if (name[1] == "#")
      try
        candidate = toobj(name);
        if (valid(candidate) && is_player(candidate))
          return candidate;
        endif
      except (ANY)
      endtry
    endif
    "Simple brute force player name scan without considering aliases.";
    for candidate in (players())
      if (candidate.name == name)
        return candidate;
      endif
    endfor
    return $failed_match;
  endmethod

  method parse_command owner: ARCH_WIZARD
    ":parse_command(@args) => {verb, args}";
    "Given the args from #0:do_login_command,";
    "  returns the actual $login verb to call and the args to use.";
    "Commands available to not-logged-in users should be located on this object and given the verb_args \"any none any\"";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    "Check for active interception (e.g., waiting for password input)";
    if (li = this:interception(player))
      return {@li, @args};
    endif
    !args && return {this.blank_command, @args};
    args[1] in {"oauth2_check", "oauth2_create", "oauth2_connect", "do_oauth_login"} && return {this.bogus_command, @args};
    if ((verb = args[1]) && !verb:is_numeric())
      for i in ({this, @ancestors(this)})
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
  endmethod

  method find_by_oauth2 owner: ARCH_WIZARD
    ":find_by_oauth2(provider, external_id)";
    "Search all players for matching oauth2_identities entry";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    {provider, external_id} = args;
    let found = $failed_match;
    for candidate in (players())
      if (is_player(candidate))
        try
          identities = candidate.oauth2_identities;
        except (E_PROPNF)
          identities = {};
        endtry
        for identity in (identities)
          if (typeof(identity) == TYPE_LIST && length(identity) == 2 && strcmp(identity[1], provider) == 0 && strcmp(identity[2], external_id) == 0)
            valid(found) && found != candidate && raise(E_INVARG, "OAuth identity is linked to multiple accounts.");
            found = candidate;
          endif
        endfor
      endif
    endfor
    return found;
  endmethod

  method _create_player owner: LOGIN
    ":_create_player(name, password, email, oauth2_identities)";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {player_name, password_value, email, oauth_entries} = args;
    cap = this.player_setup_capability;
    "cap:make_player() returns a setup capability for the new player";
    setup_cap = cap:make_player();
    "Grab the actual underlying object for other uses";
    new_player = setup_cap.delegate;
    "Configure the player while we still have temporary ownership (run_as)";
    setup_cap:set_player_flag(1);
    setup_cap:set_name_aliases(player_name, {player_name});
    if (password_value)
      setup_cap:set_password(password_value);
    endif
    if (typeof(email) == TYPE_STR)
      setup_cap:set_email_address(email);
    endif
    if (typeof(oauth_entries) == TYPE_LIST)
      setup_cap:set_oauth2_identities(oauth_entries);
    endif
    "Set the player's home to the default home (e.g., the dormitory)";
    default_home = $nothing;
    try
      default_home = this.default_home;
    except ex (E_PROPNF)
      default_home = $nothing;
    endtry
    if (valid(default_home))
      "Note: set_home is capability-gated and may not be granted; log failures.";
      try
        setup_cap:set_home(default_home);
      except ex (ANY)
        this:_server_log(tostr("_create_player: couldn't set home for ", new_player, ": ", ex));
      endtry
    endif
    "Move the player into the first room BEFORE handing ownership to the player.";
    start_room = $nothing;
    try
      start_room = $first_room;
    except ex (E_PROPNF)
      start_room = $nothing;
    endtry
    if (valid(start_room))
      try
        setup_cap:moveto(start_room);
      except ex (ANY)
        this:_server_log(tostr("_create_player: couldn't move ", new_player, " to ", start_room, ": ", ex));
      endtry
    else
      this:_server_log(tostr("_create_player: #0.first_room not valid; leaving ", new_player, " where it is"));
    endif
    "Now hand ownership to the player (self-owned)";
    setup_cap:set_owner(new_player);
    return new_player;
  endmethod

  method _server_log owner: ARCH_WIZARD
    "Write an internal login service message to the server log.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {message} = args;
    set_task_perms(this, {{"builtin_call", "server_log"}});
    server_log(message);
  endmethod

  method _password_state owner: ARCH_WIZARD
    ":_password_state(candidate, attempt) => {status, stored_password}";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {candidate, attempt} = args;
    try
      stored = candidate.password;
    except (E_PROPNF)
      return {'invalid_type, 0};
    endtry
    stored == 0 && return {'external_only, stored};
    typeof(stored) == TYPE_FLYWEIGHT || return {'invalid_type, stored};
    attempt || return {'missing, stored};
    stored:challenge(attempt) || return {'mismatch, stored};
    return {'ok, stored};
  endmethod

  method setup_new_player owner: ARCH_WIZARD
    "Set up a new player's mailbox and welcome letter. Requires wizard perms.";
    caller_perms().wizard || raise(E_PERM);
    const {new_player} = args;
    !valid(new_player) && return;
    this:_server_log(tostr("setup_new_player called for ", new_player));
    "Create welcome letter if configured";
    const letter_config = this.new_player_letter;
    this:_server_log(tostr("setup_new_player: letter_config type = ", typeof(letter_config), " length = ", typeof(letter_config) == TYPE_LIST ? length(letter_config) | 0));
    if (typeof(letter_config) == TYPE_LIST && length(letter_config) == 3)
      const {from_obj, subject, msg_lines} = letter_config;
      "Create mailbox for new player";
      const mailbox = create($mailbox, new_player);
      mailbox.name = new_player.name + "'s mailbox";
      move(mailbox, $mail_room);
      this:_server_log(tostr("setup_new_player: created mailbox ", mailbox, " owner = ", mailbox.owner));
      "Create the welcome letter";
      const letter = create($letter, from_obj);
      letter.name = subject;
      letter.author = from_obj;
      letter.addressee = new_player;
      letter.sealed = true;
      letter.sent_at = time();
      if (typeof(msg_lines) == TYPE_LIST)
        for line in (msg_lines)
          letter.text = {@letter.text, tostr(line)};
        endfor
      endif
      move(letter, mailbox);
      this:_server_log(tostr("setup_new_player: created letter ", letter, " in mailbox"));
    else
      this:_server_log("setup_new_player: letter_config not valid, skipping mailbox creation");
    endif
  endmethod

  method greet_new_player owner: ARCH_WIZARD
    "Display welcome message to a new player. Called after room confunc.";
    caller == #0 || caller_perms().wizard || raise(E_PERM);
    const {new_player} = args;
    !valid(new_player) && return;
    const title = $format.title:mk("Welcome to " + this.moo_title + "!");
    const tips_list = $format.list:mk({"Set your description: @describe me as <text>", "Set your pronouns: @pronouns they/them (or she/her, he/him, etc.)"});
    const content = $format.block:mk(title,
      $format.paragraph:mk("Try entering ", $format.code:inline("help"), " to see what kind of things you can do where you are."),
      $format.title:mk("Next steps", 4), tips_list);
    let event = $event:mk_info(new_player, content):with_audience('utility):with_presentation_hint('inset);
    new_player:inform_current(event);
    "Trigger profile setup presentation if enabled";
    if (this.post_creation_setup_enabled)
      const setup_title = this.post_creation_setup_title;
      const fields = this.post_creation_setup_fields;
      present(new_player, tostr("profile-setup-", new_player), "text/plain", "profile-setup", "", ["title" -> setup_title, "fields" -> fields]);
    endif
  endmethod

  method add_interception owner: ARCH_WIZARD
    "Add an interception for a player at login prompt.";
    "Args: {who, verbname, ...arguments}";
    "When the player types anything, parse_command routes to verbname instead.";
    caller == this || caller == #0 || caller_perms().wizard || raise(E_PERM);
    {who, verbname, @arguments} = args;
    who in this.intercepted_players && raise(E_INVARG, "Player already has an interception set.");
    this.intercepted_players = {@this.intercepted_players, who};
    this.intercepted_actions = {@this.intercepted_actions, {verbname, @arguments}};
    return 1;
  endmethod

  method delete_interception owner: ARCH_WIZARD
    "Remove an interception for a player.";
    "Args: {who}";
    caller == this || caller == #0 || caller_perms().wizard || raise(E_PERM);
    {who} = args;
    if (loc = who in this.intercepted_players)
      this.intercepted_players = listdelete(this.intercepted_players, loc);
      this.intercepted_actions = listdelete(this.intercepted_actions, loc);
      return 1;
    else
      return 0;
    endif
  endmethod

  method interception owner: ARCH_WIZARD
    "Check if a player has an active interception.";
    "Args: {who}";
    "Returns the interception action list or 0.";
    caller == this || caller == #0 || caller_perms().wizard || raise(E_PERM);
    {who} = args;
    return (loc = who in this.intercepted_players) ? this.intercepted_actions[loc] | 0;
  endmethod

  method intercepted_password owner: ARCH_WIZARD
    "Handle password input after prompting telnet user.";
    "Called via interception when user enters password.";
    caller == #0 || raise(E_PERM);
    this:delete_interception(player);
    set_connection_option(player, "client-echo", 1);
    this:_public_output("");
    try
      {candidate, ?password = ""} = args;
    except (E_ARGS)
      return 0;
    endtry
    "Re-attempt connection with the password";
    return this:connect(tostr(candidate), password);
  endmethod

  verb "?" (any none any) owner: ARCH_WIZARD flags: "rxd"
    "Handle unrecognized commands at login prompt.";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    "Collect available commands from this object";
    clist = {};
    for j in ({this, @ancestors(this)})
      for i in [1..length(verbs(j))]
        try
          info = verb_info(j, i);
          if (verb_args(j, i) == {"any", "none", "any"} && index(info[2], "x"))
            vname = info[3]:split(" ")[1];
            "Strip the * abbreviation marker";
            star = index(vname, "*");
            if (star > 0)
              vname = vname[1..star - 1] + vname[star + 1..$];
            endif
            "Skip @ prefixed aliases";
            if (vname[1] != "@" && !(vname in {"oauth2_check", "oauth2_create", "oauth2_connect"}))
              clist = {@clist, vname};
            endif
          endif
        except (ANY)
        endtry
      endfor
    endfor
    this:_public_output("I don't understand that. Available commands:");
    this:_public_output("   " + setremove(clist, "?"):join(", "));
    return 0;
  endverb

  verb "q*uit @q*uit" (any none any) owner: ARCH_WIZARD flags: "rxd"
    "Disconnect from the server.";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    boot_player(player);
    return 0;
  endverb

  verb "h*elp @h*elp" (any none any) owner: ARCH_WIZARD flags: "rxd"
    "Display help message for login screen.";
    caller == #0 || caller == this || caller_perms().wizard || raise(E_PERM);
    msg = {"## Getting Started", "", "To sign in to an existing account, use your *player name* and *password*.", "", "To create a new account, choose a unique player name and password.", "", "### Available commands", "", "- `connect <name> <password>` - Sign in to an existing account", "- `connect <name>` - Sign in (will prompt for password)", "- `create <name> <password>` - Create a new account", "- `quit` - Disconnect from the server", "", "For more detailed help once you're logged in, type `help` after connecting."};
    this:_public_output($format:authored('djot_blocks, msg:join("\n")));
    return 0;
  endverb

  verb apply_profile_setup (any none any) owner: ARCH_WIZARD flags: "rxd"
    "Apply profile setup data from client. Called via RPC.";
    "Args: {player_oid_str, profile_data_map}";
    caller == #0 || caller_perms().wizard || raise(E_PERM);
    {player_oid_str, profile_data} = args;
    target_player = player_oid_str:literal_object();
    !valid(target_player) && raise(E_INVARG, "Invalid player object");
    !is_player(target_player) && raise(E_INVARG, "Not a player object");
    "Apply pronouns if provided";
    if (profile_data["pronouns"])
      pronouns_str = profile_data["pronouns"];
      target_player:set_pronouns(pronouns_str);
    endif
    "Apply description if provided";
    if (profile_data["description"])
      desc_str = profile_data["description"];
      target_player.description = desc_str;
    endif
    return 1;
  endverb

  method _claim_oauth_identity owner: ARCH_WIZARD
    "Bind one verified identity to one account; only wizard-owned login methods may call this.";
    "The shared revision makes concurrent claims conflict. This method does not suspend.";
    caller == this && caller_perms().wizard || raise(E_PERM);
    const {account, provider, external_id} = args;
    this.oauth2_identity_version = this.oauth2_identity_version + 1;
    const linked = this:find_by_oauth2(provider, external_id);
    valid(linked) && linked != account && raise(E_INVARG, "OAuth identity is already linked to another account.");
    linked == account && return false;
    const identities = `account.oauth2_identities ! E_PROPNF => {}';
    account.oauth2_identities = {@identities, {provider, external_id}};
    return true;
  endmethod
endobject
