object ROOT [
  import_export_id -> "root",
  import_export_hierarchy -> {}
]
  name: "Root Prototype"
  location: PROTOTYPE_BOX
  owner: HACKER
  fertile: true
  readable: true

  property reaction_state_fields (owner: HACKER, flags: "rc") = [];
  property reaction_reports (owner: ARCH_WIZARD, flags: "") = [];
  property reaction_timers (owner: ARCH_WIZARD, flags: "") = [];

  property aliases (owner: HACKER, flags: "rc") = {};
  property description (owner: HACKER, flags: "rc") = "Root prototype object from which all other objects inherit.";
  property description_kind (owner: HACKER, flags: "rc") = 'literal;
  property object_documentation (owner: HACKER, flags: "rc") = 0;
  property revoked_capability_jtis (owner: ARCH_WIZARD, flags: "rc") = [];
  property thumbnail (owner: HACKER, flags: "rc") = false;

  method reference_kind owner: HACKER
    "Semantic identity for object links, inspection, and completion choices.";
    return "object";
  endmethod

  method command_action owner: ARCH_WIZARD
    "Read optional UI hints from the actual verb's metadata; aliases share one definition.";
    "The ui map accepts label, prefix (title before the target name), icon, and arguments.";
    "arguments maps dobj/iobj to field labels. Icon names are optional client decoration.";
    set_task_perms(caller_perms());
    const {names, ?target_name = ""} = args;
    let ui = `verb_metadata(this, names:words()[1], "ui") ! E_VERBNF, E_PERM => []';
    typeof(ui) == TYPE_MAP || (ui = []);
    const aliases = `verb_info(this, names:words()[1])[3] ! E_VERBNF, E_PERM => names';
    const fallback = strsub(aliases:words()[1], "*", ""):capitalize();
    const label = maphaskey(ui, "label") && typeof(ui["label"]) == TYPE_STR && ui["label"] ? ui["label"] | fallback;
    const prefix = maphaskey(ui, "prefix") && typeof(ui["prefix"]) == TYPE_STR && ui["prefix"] ? ui["prefix"] | label;
    let hint = ["label" -> label, "title" -> target_name ? prefix + " " + target_name | label];
    if (maphaskey(ui, "icon") && typeof(ui["icon"]) == TYPE_STR && ui["icon"])
      hint["icon"] = ui["icon"];
    endif
    if (maphaskey(ui, "arguments") && typeof(ui["arguments"]) == TYPE_MAP)
      hint["arguments"] = ui["arguments"];
    endif
    return hint;
  endmethod

  method message_bindings owner: HACKER
    "Declare the bindings supplied to a message property. Unknown properties have none.";
    "Override on the producer and call pass(@args) for inherited messages.";
    "Each entry has kind (object/text/content), description, and an example for previews.";
    const {property} = args;
    return [];
  endmethod

  method create owner: ARCH_WIZARD
    "Create a child of this object.";
    "";
    "  parent:create()       -- UUID-based (default)";
    "  parent:create(true)   -- anonymous (garbage collected)";
    "  parent:create(0)      -- numbered (#123 style)";
    "  parent:create(1)      -- anonymous";
    "  parent:create(2)      -- UUID";
    "";
    "Permission requires: fertile, wizard, owner, or 'create_child capability.";
    "Returns: New child object owned by caller_perms().";
    const target = typeof(this) == TYPE_FLYWEIGHT ? this.delegate | this;
    const is_fertile = target.f;
    if (!is_fertile)
      const {_, perms} = $root:_check_permissions_as(this, caller_perms(), 'create_child);
    endif
    let {?otype = 2} = args;
    if (typeof(otype) == TYPE_INT)
      "otype passed directly";
    else
      "boolean: true=anon, false=uuid";
      otype = otype ? 1 | 2;
    endif
    const new_obj = create(target, caller_perms(), otype);
    new_obj.r = 1;
    return new_obj;
  endmethod

  method destroy owner: ARCH_WIZARD
    "Destroy this object. Permission: wizard, owner, or capability.";
    const {target, perms, grants} = $root:_check_permissions_with_grants_as(this, caller_perms(), 'recycle);
    set_task_perms(perms, grants);
    recycle(target);
  endmethod

  method accept owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return this:acceptable(@args);
  endmethod

  method acceptable owner: ARCH_WIZARD
    "Returns true if the object can accept items. Called by :accept (runtime-initiated) but can also be called elsewhere in scenarios where we are just checking in-advance.";
    set_task_perms(caller_perms());
    return false;
  endmethod

  method moveto owner: ARCH_WIZARD
    "Move this object to destination. Permission: wizard, owner, or capability.";
    const {destination} = args;
    const actor = caller_perms();
    const {target, perms, grants} = $root:_check_permissions_with_grants_as(this, actor, 'move);
    set_task_perms(perms, grants);
    return `move(target, destination) ! ANY';
  endmethod

  method set_owner owner: ARCH_WIZARD
    "Validate ownership authority, then transfer the object and its `c` properties as wizard.";
    actor = caller_perms();
    {new_owner, ?suspendok = 0} = args;
    valid(new_owner) || raise(E_INVARG);
    target = typeof(this) == TYPE_FLYWEIGHT ? this.delegate | this;
    if (!(valid(actor) && (actor.wizard || actor == target.owner)))
      typeof(this) == TYPE_FLYWEIGHT || raise(E_PERM);
      maphaskey(flyslots(this), 'token) || raise(E_PERM);
      try
        claims = paseto_verify_local(this.token);
      except (E_INVARG)
        raise(E_PERM);
      endtry
      claims["target"] == target || raise(E_PERM);
      !maphaskey(claims, "exp") || time() <= claims["exp"] || raise(E_PERM);
      !$root:_capability_is_revoked(claims) || raise(E_PERM);
      'set_owner in claims["caps"] || raise(E_PERM);
    endif
    chowned_props = {};
    for definer in ({target, @ancestors(target)})
      for pname in (properties(definer))
        info = property_info(target, pname);
        if (typeof(info) == TYPE_LIST && length(info) >= 2)
          perms_string = info[2];
          if (typeof(perms_string) == TYPE_STR && index(perms_string, "c"))
            chowned_props = {@chowned_props, {pname, perms_string}};
          endif
        endif
      endfor
    endfor
    "Owner changes require wizard authority; delegated transfers cannot suspend after validation.";
    target.owner = new_owner;
    for prop in (chowned_props)
      if (suspendok && valid(actor) && actor.wizard && (ticks_left() < 5000 || seconds_left() < 2))
        suspend(0);
      endif
      set_property_info(target, prop[1], {new_owner, prop[2]});
    endfor
  endmethod

  method set_name_aliases owner: ARCH_WIZARD
    "Set this object's name and aliases. Permission: wizard, owner, or 'set_name_aliases capability.";
    const actor = caller_perms();
    const {target, perms, grants} = $root:_check_permissions_with_grants_as(this, actor, 'set_name_aliases);
    set_task_perms(perms, grants);
    const {new_name, new_aliases} = args;
    target.name = new_name;
    target.aliases = new_aliases;
  endmethod

  method contents owner: ARCH_WIZARD
    "Returns a list of the objects that are apparently inside this one.  Don't confuse this with .contents, which is a property kept consistent with .location by the server.  This verb should be used in `VR' situations, for instance when looking in a room, and does not necessarily have anything to do with the value of .contents (although the default implementation does).  `Non-VR' commands (like @contents) should look directly at .contents.";
    set_task_perms(caller_perms());
    return this.contents;
  endmethod

  method all_contents owner: ARCH_WIZARD
    "Return a list of all objects contained (at some level) by this object.";
    set_task_perms(caller_perms());
    res = {};
    for y in (this.contents)
      res = {@res, y, y:all_contents()};
    endfor
    return res;
  endmethod

  method description owner: ARCH_WIZARD
    "Return literal or structured content, parsing only explicitly selected authored Djot.";
    if (typeof(this.description) == TYPE_STR && this.description_kind != 'literal)
      return $format:authored(this.description_kind, this.description);
    endif
    return this.description;
  endmethod

  method thumbnail owner: ARCH_WIZARD
    "Return thumbnail image data for this object.";
    set_task_perms(caller_perms());
    return this.thumbnail;
  endmethod

  method set_thumbnail owner: ARCH_WIZARD
    "Set the thumbnail image for this object. Permission: owner or wizard.";
    const actor = caller_perms();
    const {target, perms, grants} = $root:_check_permissions_with_grants_as(this, actor, 'set_thumbnail);
    const {content_type, picbin} = args;
    length(picbin) > 5 * (1 << 20) && raise(E_INVARG, "Thumbnail too large (5MB max)");
    typeof(content_type) == TYPE_STR && content_type:starts_with("image/") || raise(E_TYPE);
    typeof(picbin) == TYPE_BINARY || raise(E_TYPE);
    set_task_perms(perms, grants);
    target.thumbnail = {content_type, picbin};
  endmethod

  method set_description owner: ARCH_WIZARD
    "Set this object's description. Permission: wizard, owner, or 'set_description capability.";
    const actor = caller_perms();
    const {target, perms, grants} = $root:_check_permissions_with_grants_as(this, actor, 'set_description);
    set_task_perms(perms, grants);
    const {description, ?kind = target.description_kind} = args;
    kind in {'literal, 'djot_blocks} || raise(E_INVARG, "Descriptions require literal or block Djot");
    if (typeof(description) == TYPE_STR)
      $format:authored(kind, description);
    elseif (kind != 'literal)
      raise(E_TYPE, "Authored Djot requires a source string");
    endif
    target.description = description;
    target.description_kind = kind;
  endmethod

  method name owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    "Returns the presentation name of the object.";
    return this.name;
  endmethod

  method aliases owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    "Returns the aliases of the object.";
    return this.aliases;
  endmethod

  method is_plural owner: ARCH_WIZARD
    "Returns whether this object should be treated as a plural noun.";
    "Default for system objects: false (singular).";
    return false;
  endmethod

  method is_countable owner: ARCH_WIZARD
    "Returns whether this object is countable in English grammar.";
    "Default for system objects: false (mass nouns, proper nouns, etc).";
    return false;
  endmethod

  method is_proper_noun owner: ARCH_WIZARD
    "Returns whether this object should be treated as a proper noun.";
    "Default for system objects: true (all system objects are proper nouns).";
    return true;
  endmethod

  method look_self owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    return $look:mk(this, @this.contents);
  endmethod

  method all_verbs owner: ARCH_WIZARD
    set_task_perms(caller_perms());
    "Recurse up the inheritance hierarchy, getting a list of all verbs.";
    what = this;
    verbs = {};
    while (valid(what))
      verbs = {@verbs(what) || {}, @verbs};
      what = parent(what);
    endwhile
    return verbs;
  endmethod

  method all_properties owner: ARCH_WIZARD
    "Recurse up the inheritance hierarchy, getting a list of all properties.";
    set_task_perms(caller_perms());
    what = this;
    result = {};
    while (valid(what))
      props = `properties(what) ! E_PERM => {}';
      result = {@props, @result};
      what = parent(what);
    endwhile
    return result;
  endmethod

  method all_command_verbs owner: ARCH_WIZARD
    "Get all command verbs (readable, not 'this none this') from this object and ancestors.";
    "Returns list of {verb_name, definer_object, dobj, prep, iobj} for each command verb.";
    set_task_perms(caller_perms());
    result = {};
    "Walk inheritance chain";
    for definer in ({this, @ancestors(this)})
      "Get verbs defined on this specific object";
      for verb_name in (verbs(definer))
        "Get verb info to check flags";
        {verb_owner, verb_flags, verb_names} = verb_info(definer, verb_name);
        "Skip non-readable verbs";
        if (!index(verb_flags, "r"))
          continue;
        endif
        "Get verb args to check if it's a command verb";
        {dobj, prep, iobj} = verb_args(definer, verb_name);
        "Skip internal 'this none this' verbs";
        if (dobj == "this" && prep == "none" && iobj == "this")
          continue;
        endif
        "Add to result list";
        result = {@result, {verb_name, definer, dobj, prep, iobj}};
      endfor
    endfor
    return result;
  endmethod

  method branches owner: ARCH_WIZARD
    ":branches(object) => list of all descendants of this object which have children.";
    set_task_perms(caller_perms());
    if (kids = children(object = this))
      s = {object};
      for k in (kids)
        s = {@s, @k:branches()};
      endfor
      return s;
    else
      return {};
    endif
  endmethod

  method find_verb_definer owner: ARCH_WIZARD
    "Find verb on object or its ancestors, returning the object that actually defines the verb.";
    "Uses ancestors() builtin and verb_info() to handle aliases, wildcards, and inheritance.";
    "Usage: obj:find_verb_definer(verb_name)";
    set_task_perms(caller_perms());
    {verb_name} = args;
    "Check this object first";
    try
      verb_info(this, verb_name);
      return this;
    except (E_VERBNF)
    endtry
    "Then check ancestors";
    ancestor_list = ancestors(this);
    for ancestor in (ancestor_list)
      try
        verb_info(ancestor, verb_name);
        return ancestor;
      except (E_VERBNF)
        continue;
      endtry
    endfor
    return #-1;
  endmethod

  method estimated_size_bytes owner: ARCH_WIZARD
    "Return a spitball estimate of the in-memory size / on-disk size of this object.";
    "No guarantee of accuracy and this computation is relatively expensive so use sparingly.";
    "Caller must own the object or have the arcane powers of a wizard.";
    caller == this.owner || caller.wizard || raise(E_PERM);
    return object_bytes(this);
  endmethod

  method issue_capability owner: ARCH_WIZARD
    "Issue an unforgeable capability flyweight for delegating specific permissions.";
    "";
    "Capabilities implement object-capability security (E-rights model) where possession";
    "of the flyweight grants authority. The capability is cryptographically signed using";
    "PASETO V4.Local tokens, making them unforgeable and tamper-proof.";
    "";
    "Args:";
    "  target        - Object the capability grants access to (becomes flyweight delegate)";
    "  cap_list      - List of capability symbols (e.g., {'read, 'write, 'enter})";
    "  ?expiration   - Optional Unix timestamp when capability expires";
    "  ?run_as       - Optional object to elevate permissions to (caller or player only)";
    "  ?key          - Optional custom signing key (for testing; default uses server key)";
    "";
    "Returns: Flyweight <target, [token -> paseto_token]>";
    "";
    "Security:";
    "  - Only object owner or wizard can issue capabilities for an object";
    "  - Tokens are signed with server's symmetric key (wizard-only access)";
    "  - Token includes: target, caps, issued_at, granted_by, unique_id, optional exp/run_as";
    "  - Possession of flyweight grants bearer authority (protect like passwords)";
    "";
    "Example:";
    "  key = room:issue_capability(locked_room, {'enter), time() + 3600);";
    "  move(key, player);  # Give player a 1-hour access key";
    "  ";
    "  setup_cap = $root:issue_capability(new_player, {'set_owner, 'set_password});";
    "  setup_cap:set_owner(new_player);  # Capability-protected setup";
    {target, cap_list, ?expiration = 0, ?run_as = 0, ?key = 0} = args;
    "Only owner or wizard can issue";
    !caller_perms().wizard && caller_perms() != target.owner && raise(E_PERM);
    "Build claims map - PASETO handles symbols/objects natively via __type_* tags";
    jti = uuid();
    claims = ['target -> target, 'caps -> cap_list, 'iat -> time(), 'granted_by -> caller_perms(), 'jti -> jti];
    "Add optional expiration";
    if (expiration)
      claims['exp] = expiration;
    endif
    "Add run_as if provided - issuer can grant run_as for self or player";
    "Note: Check run_as != 0, not truthiness, because objects are falsy in MOO";
    if (run_as != 0)
      run_as == caller_perms() || run_as == player || raise(E_PERM);
      claims['run_as] = run_as;
    endif
    "Create server authority PASETO token (wizard-only builtin)";
    token = key ? paseto_make_local(claims, key) | paseto_make_local(claims);
    return <target, .token = token, .jti = jti>;
  endmethod

  method merge_capability owner: ARCH_WIZARD
    "Merge two capability flyweights for the same target into one with combined permissions.";
    caller_perms().wizard || raise(E_PERM);
    const {cap1, cap2, ?key = 0} = args;
    "Both must be flyweights with tokens";
    typeof(cap1) == TYPE_FLYWEIGHT && typeof(cap2) == TYPE_FLYWEIGHT || raise(E_TYPE);
    maphaskey(flyslots(cap1), 'token) && maphaskey(flyslots(cap2), 'token) || raise(E_INVARG);
    "Both must be for the same target";
    cap1.delegate == cap2.delegate || raise(E_INVARG, "Capabilities must be for same target");
    const target = cap1.delegate;
    "Decode both tokens";
    const claims1 = key ? paseto_verify_local(cap1.token, key) | paseto_verify_local(cap1.token);
    const claims2 = key ? paseto_verify_local(cap2.token, key) | paseto_verify_local(cap2.token);
    claims1["target"] == target && claims2["target"] == target || raise(E_PERM);
    $root:_capability_is_revoked(claims1) && raise(E_PERM);
    $root:_capability_is_revoked(claims2) && raise(E_PERM);
    maphaskey(claims1, "exp") && time() > claims1["exp"] && raise(E_PERM);
    maphaskey(claims2, "exp") && time() > claims2["exp"] && raise(E_PERM);
    "Combine capability lists (remove duplicates)";
    const all_caps = {@claims1["caps"], @claims2["caps"]};
    let unique_caps = {};
    for cap in (all_caps)
      !(cap in unique_caps) && (unique_caps = {@unique_caps, cap});
    endfor
    "Take the later expiration if any";
    let exp = 0;
    maphaskey(claims1, "exp") && (exp = claims1["exp"]);
    maphaskey(claims2, "exp") && claims2["exp"] > exp && (exp = claims2["exp"]);
    "Take run_as if either has it (prefer cap1) - comes back as object";
    let run_as = 0;
    if (maphaskey(claims1, "run_as"))
      run_as = claims1["run_as"];
    elseif (maphaskey(claims2, "run_as"))
      run_as = claims2["run_as"];
    endif
    "Caps come back as symbols directly - unique_caps is already a list of symbols";
    "Issue new merged capability";
    return this:issue_capability(target, unique_caps, exp, run_as, key);
  endmethod

  method grant_capability owner: ARCH_WIZARD
    "Grant capabilities for target_obj to grantee, storing in specified category.";
    {target_obj, cap_list, grantee, category, ?key = 0} = args;
    "Permission: wizard, owner of target_obj, or TODO: 'grant capability";
    caller_perms().wizard || caller_perms() == target_obj.owner || raise(E_PERM);
    "Validate category is a symbol";
    typeof(category) == TYPE_SYM || raise(E_TYPE);
    "Construct property name from category";
    prop_name = "grants_" + tostr(category);
    "Check that grantee has this grants bucket";
    grants_map = 0;
    try
      grants_map = grantee.(prop_name);
    except (E_PROPNF)
      raise(E_INVARG, tostr(grantee) + " cannot accept grants of category " + tostr(category) + " (missing property: " + prop_name + ")");
    endtry
    typeof(grants_map) == TYPE_MAP || raise(E_INVARG, tostr(grantee) + "." + prop_name + " must be a map");
    "Issue new capability";
    new_cap = target_obj:issue_capability(target_obj, cap_list, 0, 0, key);
    "Check if grantee already has a grant for this object";
    if (maphaskey(grants_map, target_obj))
      "Merge with existing grant";
      old_cap = grants_map[target_obj];
      new_cap = $root:merge_capability(old_cap, new_cap, key);
      $root:_revoke_capability_token(old_cap, key);
    endif
    "Store the grant";
    grants_map[target_obj] = new_cap;
    grantee.(prop_name) = grants_map;
    "Notify the grantee if they're a player";
    if (is_player(grantee))
      grant_display = $grant_utils:format_grant_with_name(target_obj, category, cap_list);
      message = "You have been granted " + grant_display + ".";
      grantee:inform_all($event:mk_info(grantee, message));
    endif
    return new_cap;
  endmethod

  method revoke_capability owner: ARCH_WIZARD
    "Revoke a stored capability grant for target_obj from grantee in category.";
    {target_obj, grantee, category} = args;
    caller_perms().wizard || caller_perms() == target_obj.owner || raise(E_PERM);
    typeof(category) == TYPE_SYM || raise(E_TYPE);
    prop_name = "grants_" + tostr(category);
    try
      grants_map = grantee.(prop_name);
    except (E_PROPNF)
      raise(E_INVARG, tostr(grantee) + " cannot accept grants of category " + tostr(category) + " (missing property: " + prop_name + ")");
    endtry
    typeof(grants_map) == TYPE_MAP || raise(E_INVARG, tostr(grantee) + "." + prop_name + " must be a map");
    maphaskey(grants_map, target_obj) || return false;
    old_cap = grants_map[target_obj];
    this:_revoke_capability_token(old_cap);
    grantee.(prop_name) = mapdelete(grants_map, target_obj);
    return true;
  endmethod

  method challenge_for owner: ARCH_WIZARD
    "Validate a capability and verify it grants the required permissions.";
    "";
    "Called on a capability flyweight to check if it grants specific capabilities.";
    "Performs cryptographic verification of the PASETO token, validates expiration,";
    "and checks target binding.";
    "";
    "Args: Variable number of capability symbols to require (e.g., 'read, 'write)";
    "";
    "Returns: {delegate, run_as_object} where run_as is from token or $hacker";
    "";
    "Raises: E_PERM if:";
    "  - this is not a flyweight";
    "  - token signature is invalid or tampered";
    "  - token has expired";
    "  - target binding doesn't match flyweight delegate";
    "  - any required capability is not granted";
    "";
    "Example:";
    "  {target, perms} = this:challenge_for('enter);";
    "  set_task_perms(perms);";
    return $root:_challenge_subject(this, args, 0);
  endmethod

  method challenge_for_with_key owner: ARCH_WIZARD
    "Validate a capability using a custom signing key (for testing).";
    "";
    "Like challenge_for() but accepts a custom PASETO signing key instead of";
    "using the server's symmetric key. Primarily for testing scenarios.";
    "";
    "Args:";
    "  caps_list  - List of capability symbols (e.g., {'read, 'write})";
    "  key        - Custom PASETO signing key (base64-encoded 32-byte string)";
    "";
    "Returns: {delegate, run_as_object} where run_as is from token or $hacker";
    "";
    "Example:";
    "  test_key = \"dGVzdHRlc3R0ZXN0dGVzdHRlc3R0ZXN0dGVzdHRlc3Q=\";";
    "  {target, perms} = cap:challenge_for_with_key({'read}, test_key);";
    const {caps_list, key} = args;
    return $root:_challenge_subject(this, caps_list, key);
  endmethod

  method require_caller owner: HACKER
    "Verify that caller is the expected object (or a flyweight with that object as delegate).";
    "Raises E_PERM if check fails, otherwise returns normally.";
    "Usage: $root:require_caller(this);";
    {expected} = args;
    if (caller == expected)
      return;
    endif
    if (typeof(caller) == TYPE_FLYWEIGHT && caller.delegate == expected)
      return;
    endif
    raise(E_PERM);
  endmethod

  method check_permissions_as owner: HACKER
    "Check owner, wizard, or bearer authority; privileged callers use fixed root dispatch.";
    return $root:_check_permissions_as(this, @args);
  endmethod

  method _check_permissions_as owner: HACKER
    "Validate the explicit subject without invoking its overridable authorization methods.";
    const {subject, actor, @required_caps} = args;
    const target = typeof(subject) == TYPE_FLYWEIGHT ? subject.delegate | subject;
    if (valid(actor) && (actor.wizard || actor == target.owner))
      return {target, actor};
    endif
    typeof(subject) == TYPE_FLYWEIGHT || raise(E_PERM);
    return $root:_challenge_subject(subject, required_caps, 0);
  endmethod

  method check_permissions_with_grants_as owner: HACKER
    "Check owner, wizard, or bearer authority and return the required runtime grants.";
    return $root:_check_permissions_with_grants_as(this, @args);
  endmethod

  method _check_permissions_with_grants_as owner: HACKER
    "Map verified capability authority to grants without dispatching on the target.";
    const {subject, actor, @required_caps} = args;
    const target = typeof(subject) == TYPE_FLYWEIGHT ? subject.delegate | subject;
    if (valid(actor) && (actor.wizard || actor == target.owner))
      return {target, actor, {}};
    endif
    typeof(subject) == TYPE_FLYWEIGHT || raise(E_PERM);
    let {validated_target, perms} = $root:_challenge_subject(subject, required_caps, 0);
    if (valid(actor) && perms == $hacker)
      perms = actor;
    endif
    return {validated_target, perms, $root:_runtime_grants_for_target(validated_target, required_caps)};
  endmethod

  method _runtime_grants_for_caps owner: HACKER
    "Return low-level runtime grants for supported root object mutation capabilities.";
    return $root:_runtime_grants_for_target(this, @args);
  endmethod

  method _runtime_grants_for_target owner: HACKER
    "Return only the runtime grants mapped to the validated target and requested capabilities.";
    const {target, caps} = args;
    let grants = {};
    for cap in (caps)
      if (cap == 'move)
        grants = {@grants, {"object_move", target}};
      elseif (cap == 'recycle)
        grants = {@grants, {"object_recycle", target}};
      elseif (cap == 'set_description)
        grants = {@grants, {"property_write", target, "description"},
          {"property_write", target, "description_kind"}};
      elseif (cap == 'set_name_aliases)
        grants = {@grants, {"object_rename", target}, {"property_write", target, "aliases"}};
      elseif (cap == 'set_owner)
        grants = {@grants, {"property_write", target, "owner"}};
      elseif (cap == 'set_thumbnail)
        grants = {@grants, {"property_write", target, "thumbnail"}};
      elseif (cap == 'set_api_key)
        grants = {@grants, {"property_write", target, "api_key"}};
      else
        raise(E_PERM, "No runtime grant mapping for capability " + tostr(cap));
      endif
    endfor
    return grants;
  endmethod

  method _capability_challenge owner: ARCH_WIZARD
    "Validate a bearer capability through fixed root dispatch.";
    const {required_caps, key} = args;
    return $root:_challenge_subject(this, required_caps, key);
  endmethod

  method _challenge_subject owner: ARCH_WIZARD
    "Verify signature, target binding, expiry, revocation, and capability subset without delegate callbacks.";
    const {subject, required_caps, key} = args;
    typeof(subject) == TYPE_FLYWEIGHT || raise(E_PERM);
    maphaskey(flyslots(subject), 'token) || raise(E_PERM);
    let claims;
    try
      claims = key ? paseto_verify_local(subject.token, key) | paseto_verify_local(subject.token);
    except (E_INVARG)
      raise(E_PERM);
    endtry
    subject.delegate == claims["target"] || raise(E_PERM);
    maphaskey(claims, "exp") && time() > claims["exp"] && raise(E_PERM);
    $root:_capability_is_revoked(claims) && raise(E_PERM);
    for required in (required_caps)
      required in claims["caps"] || raise(E_PERM);
    endfor
    const run_as = `claims["run_as"] ! E_RANGE => $hacker';
    return {subject.delegate, run_as};
  endmethod

  method _capability_jti owner: ARCH_WIZARD
    "Return a capability token id from its flyweight slot or decoded claims.";
    {cap, ?key = 0} = args;
    if (typeof(cap) != TYPE_FLYWEIGHT)
      return 0;
    endif
    jti = `cap.jti ! E_PROPNF, E_TYPE => 0';
    if (jti)
      return jti;
    endif
    if (!maphaskey(flyslots(cap), 'token))
      return 0;
    endif
    try
      claims = key ? paseto_verify_local(cap.token, key) | paseto_verify_local(cap.token);
    except (ANY)
      return 0;
    endtry
    return `claims["jti"] ! E_RANGE, E_TYPE => 0';
  endmethod

  method _revoke_capability_token owner: ARCH_WIZARD
    "Record a capability token id as revoked so copied bearer tokens fail challenge.";
    caller == this || caller_perms().wizard || raise(E_PERM);
    {cap, ?key = 0} = args;
    jti = this:_capability_jti(cap, key);
    if (!jti)
      return false;
    endif
    revoked = this.revoked_capability_jtis;
    if (typeof(revoked) != TYPE_MAP)
      revoked = [];
    endif
    revoked[jti] = time();
    this.revoked_capability_jtis = revoked;
    return true;
  endmethod

  method _capability_is_revoked owner: ARCH_WIZARD
    "Return true if decoded capability claims identify a revoked token.";
    {claims} = args;
    if (typeof(claims) != TYPE_MAP)
      return false;
    endif
    jti = `claims["jti"] ! E_RANGE, E_TYPE => 0';
    if (!jti)
      return false;
    endif
    revoked = `$root.revoked_capability_jtis ! E_PROPNF, E_PERM => []';
    if (typeof(revoked) != TYPE_MAP)
      return false;
    endif
    return maphaskey(revoked, jti);
  endmethod

  method is_actor owner: HACKER
    "Return whether this object is an actor (player or NPC). Override in descendants.";
    return false;
  endmethod

  method display_name owner: HACKER
    "Return the display name for this object. Defaults to :name() but can be overridden for richer descriptions.";
    return this:name();
  endmethod

  method check_property_exists owner: ARCH_WIZARD
    "Check if a property exists on this object.";
    "Returns false if property doesn't exist or if the user does not have permissions to view it. Otherwise returns true.";
    "Usage: $root:check_property_exists(target_obj, prop_name)";
    {prop_name} = args;
    set_task_perms(caller_perms());
    return `property_info(this, prop_name) ! ANY => false' ? true | false;
  endmethod

  method suggestions owner: ARCH_WIZARD
    "Read-only ranked completion using a shared argument context.";
    const {source, query, ?raw_context = [], ?limit = 12} = args;
    valid(player) && is_player(player) || raise(E_PERM);
    typeof(source) == TYPE_STR && typeof(query) == TYPE_STR && typeof(limit) == TYPE_INT || raise(E_TYPE);
    length(source) <= 64 && length(query) <= 256 && limit >= 1 && limit <= 50 || raise(E_INVARG);
    const context = $match:normalize_input_context(raw_context);
    set_task_perms(player);
    return $match:rank_suggestions(this:suggestion_candidates(source, query, context), query, limit);
  endmethod

  method suggestion_candidates owner: ARCH_WIZARD
    "Use the same discovery scope and command matcher for ranked and explicit reference queries.";
    set_task_perms(caller_perms());
    const {source, ?query = "", ?context = []} = args;
    const template = `context["template"] ! E_RANGE => ""';
    const scope = player:match_environment(template, ['scope -> source, 'target -> this]);
    const candidates = $match:object_suggestions(scope);
    !length(context) && return candidates;
    return $match:matching_suggestions(candidates, context, {@player:match_environment(template), @scope}, player:command_environment());
  endmethod

  method suggestion_eligibility owner: ARCH_WIZARD
    "Check a bounded batch of explicit references against the complete candidate scope, without ranking or execution.";
    const {source, references, ?raw_context = []} = args;
    valid(player) && is_player(player) || raise(E_PERM);
    typeof(source) == TYPE_STR && length(source) <= 64 && typeof(references) == TYPE_LIST && length(references) <= 64 || raise(E_INVARG);
    const context = $match:normalize_input_context(raw_context);
    set_task_perms(player);
    let eligible = [];
    for candidate in (this:suggestion_candidates(source, "", context))
      const object = `toobj(candidate["value"]) ! E_TYPE, E_INVARG, E_RANGE => $nothing';
      if (valid(object) && tostr(object) == candidate["value"])
        eligible[$url_utils:to_curie_str(object)] = candidate;
      endif
    endfor
    let result = [];
    for reference in (references)
      typeof(reference) == TYPE_STR && length(reference) <= 64 || raise(E_INVARG);
      if (maphaskey(eligible, reference))
        const candidate = eligible[reference];
        result[reference] = ["eligible" -> true, "label" -> candidate["label"], "value" -> candidate["value"]];
      else
        result[reference] = ["eligible" -> false, "reason" -> "That reference is not available for this argument."];
      endif
    endfor
    return result;
  endmethod

  method match_scope_for owner: ARCH_WIZARD
    "Contribute visible objects and aliases to an actor's environmental search.";
    return {};
  endmethod

  method help_topic owner: ARCH_WIZARD
    "Read an exact topic from this provider without resolving against the viewer's current room.";
    const {topic_name} = args;
    const actor = player;
    valid(actor) && is_player(actor) || raise(E_PERM);
    this.r || this.owner == actor || actor.wizard || raise(E_PERM);
    typeof(topic_name) == TYPE_STR && length(topic_name) <= 256 || raise(E_INVARG);
    set_task_perms(actor);
    const topics = this:help_topics(actor, "");
    typeof(topics) == TYPE_LIST || raise(E_INVARG, "Help topic is unavailable");
    for topic in (topics)
      if (typeof(topic) == TYPE_FLYWEIGHT && topic.name == topic_name)
        const event = $event:mk_info(actor, $format.block:mk(@topic:render_prose()));
        const rendered = $document:render(event:resolve(actor), 'text_djot);
        return ["title" -> topic.name, "content" -> rendered['body], "annotations" -> rendered['annotations]];
      endif
    endfor
    raise(E_INVARG, "Help topic is unavailable");
  endmethod

  method inspection owner: ARCH_WIZARD
    "Describe an object and its command suggestions for a viewer. Availability is advisory.";
    const {?who = player} = args;
    return ["title" -> this:name(), "objectKind" -> this:reference_kind(), "description" -> this:description(), "state" -> this:inspection_state(who), "actions" -> this:inspection_actions(who)];
  endmethod

  method inspection_state owner: ARCH_WIZARD
    "Return short, observable state labels for the inspector.";
    return {};
  endmethod

  method inspection_actions owner: ARCH_WIZARD
    "Return labelled commands; command parsing and execution remain authoritative.";
    const {?who = player} = args;
    return {["id" -> "examine", "label" -> "Examine", "command" -> "examine " + tostr(this), "action" -> $player:command_action("examine", this:name())], @this:inspection_commands(who)};
  endmethod

  method inspection_commands owner: ARCH_WIZARD
    "Suggest simple public command signatures, retaining names and prepositions from the verbs.";
    const {?who = player} = args;
    let actions = {};
    let seen = {"examine", "look", "inspect", "inspection", "get", "take", "drop"};
    const ref = tostr(this);
    for spec in (this:usable_verbs())
      const {names, definer, direct, prep_spec, indirect} = spec;
      const name = strsub(names:words()[1], "*", "");
      if (name in seen)
        continue;
      endif
      const prep = prep_spec == "none" ? "" | prep_spec:split("/")[1];
      "An unconstrained preposition needs an authored command rather than a guess.";
      if (prep_spec == "any")
        continue;
      endif
      let command = "";
      let input_label = "";
      if (direct == "this" && prep_spec == "none" && indirect == "none")
        command = name + " " + ref;
      elseif (direct == "this" && indirect == "any" && prep)
        command = name + " " + ref + " " + prep + " {input}";
        input_label = "With what?";
      elseif (indirect == "this" && direct == "any" && prep)
        command = name + " {input} " + prep + " " + ref;
        input_label = "Which item?";
      elseif (indirect == "this" && direct == "none" && prep)
        command = name + " " + prep + " " + ref;
      endif
      if (!command)
        continue;
      endif
      seen = {@seen, name};
      const hint = definer:command_action(names, this:name());
      let action = ["id" -> name, "label" -> hint["label"], "command" -> command, "action" -> hint];
      if (input_label)
        const slot = direct == "any" ? "dobj" | "iobj";
        const labels = maphaskey(hint, "arguments") ? hint["arguments"] | [];
        if (maphaskey(labels, slot) && typeof(labels[slot]) == TYPE_STR && labels[slot])
          input_label = labels[slot];
        else
          input_label = "Object";
        endif
        action["input"] = ["label" -> input_label, "placeholder" -> "Find an item\u2026", "suggestions" -> ["provider" -> $url_utils:to_curie_str(who), "source" -> "nearby"]];
      endif
      actions = {@actions, action};
    endfor
    return actions;
  endmethod

  method usable_verbs owner: ARCH_WIZARD
    "Get verbs that can use this object as a target (dobj or iobj).";
    "Returns list of {verb_name, definer_object, dobj, prep, iobj} for verbs that accept 'any' or 'this'.";
    "Useful for determine what commands a player can perform on/with an object.";
    set_task_perms(caller_perms());
    result = {};
    "Walk inheritance chain starting from this object";
    for definer in ({this, @ancestors(this)})
      "Get all verbs defined on this level - skip if not readable";
      verb_list = `verbs(definer) ! E_PERM => {}';
      for verb_name in (verb_list)
        "Get verb info to check flags and signature";
        info = `verb_info(definer, verb_name) ! E_PERM => 0';
        if (!info)
          continue;
        endif
        {verb_owner, verb_flags, verb_names} = info;
        "Skip non-readable verbs";
        if (!index(verb_flags, "r"))
          continue;
        endif
        "Get verb signature";
        {dobj, prep, iobj} = verb_args(definer, verb_name);
        "Skip internal utility verbs like 'this none this' (methods, not commands)";
        if (dobj == "this" && prep == "none" && iobj == "this")
          continue;
        endif
        "Include verbs that have 'this' as dobj or iobj - excludes 'any any any' verbs";
        if (dobj == "this" || iobj == "this" || dobj == "any" || iobj == "any")
          "But exclude verbs that are 'any any any' (not useful for examining this object)";
          if (!(dobj == "any" && iobj == "any"))
            result = {@result, {verb_name, definer, dobj, prep, iobj}};
          endif
        endif
      endfor
    endfor
    return result;
  endmethod

  method examination owner: ARCH_WIZARD
    "Return structured examination data about this object as a flyweight.";
    "Contains slots: object_ref, name, aliases, owner, parent, description, verbs (usable ones), location, contents.";
    "Subclasses should override this to customize or extend the examination data.";
    "Usage: examination = obj:examination();";
    "Returns: <$examination, [slots...]>";
    set_task_perms(caller_perms());
    "Get basic properties";
    obj_name = this:name();
    obj_aliases = this:aliases();
    obj_description = this:description();
    obj_owner = this.owner;
    obj_parent = parent(this);
    obj_location = this.location;
    "Get verbs that can use this object as a target";
    usable = this:usable_verbs();
    "Get contents";
    obj_contents = this:contents();
    return <$examination, .object_ref = this, .name = obj_name, .aliases = obj_aliases, .description = obj_description, .owner = obj_owner, .parent = obj_parent, .location = obj_location, .verbs = usable, .contents = obj_contents>;
  endmethod

  method object_help owner: ARCH_WIZARD
    "Return formatted help for this object. Returns a $format flyweight or 0 if no help available.";
    "Checks for .object_help property and formats it if found.";
    set_task_perms(caller_perms());
    help_text = `this.object_help ! ANY => 0';
    if (!help_text || help_text == 0)
      return 0;
    endif
    "Format as djot content";
    if (typeof(help_text) == TYPE_LIST)
      return $format.block:mk(this:display_name(), @help_text);
    else
      return $format.block:mk(this:display_name(), {help_text});
    endif
  endmethod

  method fact_owner_is owner: HACKER
    "Rule predicate: Does player_obj own this object?";
    {thing, player_obj} = args;
    return thing.owner == player_obj;
  endmethod

  method fact_location_is owner: HACKER
    "Rule predicate: Is this object at location loc?";
    {thing, loc} = args;
    return thing.location == loc;
  endmethod

  method fact_contains owner: HACKER
    "Rule predicate: Does this object contain thing?";
    {container, thing} = args;
    return thing.location == container;
  endmethod

  method fact_is owner: HACKER
    "Rule predicate: Is obj1 the same object as obj2?";
    {obj1, obj2} = args;
    return obj1 == obj2;
  endmethod

  method fact_isa owner: HACKER
    "Rule predicate: Is target a descendant of proto?";
    {target, proto} = args;
    return typeof(target) == TYPE_OBJ && valid(target) && isa(target, proto);
  endmethod

  method get_reactions owner: ARCH_WIZARD
    "Gather installed reactions from this object's readable reaction properties.";
    return $root:_get_reactions_for(this, caller_perms());
  endmethod

  method _get_reactions_for owner: ARCH_WIZARD
    "Read canonical reaction properties on a fixed host under the supplied caller authority.";
    const {host, actor, ?include_names = false} = args;
    actor == caller_perms() || caller_perms().wizard || raise(E_PERM);
    set_task_perms(actor);
    typeof(host) == TYPE_OBJ && valid(host) || raise(E_INVARG);
    let result = {};
    let names = {};
    for ancestor in ({host, @ancestors(host)})
      for name in (`properties(ancestor) ! E_PERM => {}')
        if (!(name in names))
          names = {@names, name};
        endif
      endfor
    endfor
    for name in (names)
      if (!tostr(name):ends_with("_reaction"))
        continue;
      endif
      try
        const reaction = host.(name);
        if (typeof(reaction) == TYPE_FLYWEIGHT && reaction.delegate == $reaction)
          result = {@result, include_names ? {name, reaction} | reaction};
        endif
      except (E_PROPNF, E_PERM)
        continue;
      endtry
    endfor
    return result;
  endmethod

  method check_actor owner: ARCH_WIZARD
    "Require the caller to act as itself, as an owned NPC, or with wizard authority.";
    const {who, ?principal = caller_perms()} = args;
    (principal == caller_perms() || caller_perms().wizard) || raise(E_PERM);
    (typeof(who) == TYPE_OBJ && valid(who) && isa(who, $actor))
      || raise(E_INVARG, "The action needs a person or an NPC.");
    (principal == who || principal.wizard
      || (principal == who.owner && !is_player(who))) || raise(E_PERM, "You cannot act as that person.");
    return true;
  endmethod

  method fire_trigger owner: ARCH_WIZARD
    "Request a trigger as the host owner, a wizard, or a verified movement callback.";
    const {trigger_name, ?initial_context = []} = args;
    typeof(this) == TYPE_OBJ && valid(this) || raise(E_INVARG);
    typeof(trigger_name) == TYPE_SYM && typeof(initial_context) == TYPE_MAP || raise(E_INVARG);
    const principal = caller_perms();
    const source = callers()[1];
    const movement = source[1] == this && source[4] == $root && source[2] in {"enterfunc", "exitfunc"};
    principal == this.owner || principal.wizard || movement || raise(E_PERM, "Only the host owner can request a reaction trigger");
    return $reaction:_dispatch(this, trigger_name, initial_context);
  endmethod

  method _is_movement_callback owner: HACKER
    "Recognize native move/recycle callbacks through canonical room/root pass frames.";
    const {host, callback} = args;
    for frame in (callers())
      if (frame[1] == host && frame[2] == callback && frame[4] in {$root, $room})
        continue;
      endif
      if (frame[4] == #-1 && frame[2] == "pass")
        continue;
      endif
      return frame[4] == #-1 && frame[2] in {"move", "recycle"};
    endfor
    return false;
  endmethod

  method enterfunc owner: HACKER
    "Publish entry only for an actual movement callback.";
    const {who} = args;
    $root:_is_movement_callback(this, "enterfunc") || return;
    this:fire_trigger('on_enter, ['Who -> who, 'Actor -> who]);
  endmethod

  method exitfunc owner: HACKER
    "Publish departure only for an actual movement callback.";
    const {who} = args;
    $root:_is_movement_callback(this, "exitfunc") || return;
    this:fire_trigger('on_exit, ['Who -> who, 'Actor -> who]);
  endmethod

  method initialize owner: ARCH_WIZARD
    "Base initialization hook called after object creation.";
    "Subclasses should call pass() to preserve the initialization chain.";
  endmethod

  method test_can_create_unrooted_object owner: ARCH_WIZARD
    "Creating with no parent and no owner should make the object own itself.";
    scratch = create($nothing, $nothing);
    try
      scratch.owner == scratch || raise(E_ASSERT, "unrooted object should own itself");
    finally
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod

  method test_programmer_can_create_child_of_root owner: ARCH_WIZARD
    "Creating a direct child of $root should make the current task perms its owner.";
    scratch = create($root);
    try
      scratch.owner == $arch_wizard || raise(E_ASSERT, "root child should be owned by arch wizard");
    finally
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod

  method test_capabilities owner: ARCH_WIZARD
    "Test capability issuance and challenge with custom test key";
    test_key = "dGVzdHRlc3R0ZXN0dGVzdHRlc3R0ZXN0dGVzdHRlc3Q=";
    "Test 1: Issue capability with custom key";
    cap = this:issue_capability(this, {'read}, 0, 0, test_key);
    typeof(cap) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Cap should be flyweight");
    cap.delegate == this || raise(E_ASSERT, "Cap delegate should be this");
    maphaskey(flyslots(cap), 'token) || raise(E_ASSERT, "Cap should have token slot");
    "Test 2: Challenge returns {delegate, run_as}";
    {target, run_as} = cap:challenge_for_with_key({'read}, test_key);
    typeof(target) == TYPE_OBJ || raise(E_ASSERT, "Target should be OBJ");
    target == this || raise(E_ASSERT, "Target should be this");
    typeof(run_as) == TYPE_OBJ || raise(E_ASSERT, "run_as should be OBJ");
    run_as == $hacker || raise(E_ASSERT, "run_as should be $hacker");
    "Test 3: Multiple capabilities";
    multi_cap = this:issue_capability(this, {'read, 'write, 'execute}, 0, 0, test_key);
    multi_cap:challenge_for_with_key({'read, 'write}, test_key);
    "Should succeed - all required caps present";
    "Test 4: Expiration check";
    expired_cap = this:issue_capability(this, {'read}, time() - 1, 0, test_key);
    expired_valid = false;
    try
      expired_cap:challenge_for_with_key({'read}, test_key);
      expired_valid = true;
    except (E_PERM)
    endtry
    !expired_valid || raise(E_ASSERT("Expired capability should have raised E_PERM"));
    "Test 5: Missing capability";
    read_cap = this:issue_capability(this, {'read}, 0, 0, test_key);
    write_denied = false;
    try
      read_cap:challenge_for_with_key({'write}, test_key);
      write_denied = true;
    except (E_PERM)
    endtry
    !write_denied || raise(E_ASSERT("Missing capability should have raised E_PERM"));
    "Test 6: run_as claim";
    run_as_cap = this:issue_capability(this, {'read}, 0, $arch_wizard, test_key);
    {target2, run_as_obj} = run_as_cap:challenge_for_with_key({'read}, test_key);
    target2 == this || raise(E_ASSERT, "run_as cap target should be this");
    run_as_obj == $arch_wizard || raise(E_ASSERT, "run_as_obj should be $arch_wizard");
    return true;
  endmethod

  method test_merge_capability owner: ARCH_WIZARD
    "Test merging two capabilities for the same target";
    test_key = "dGVzdHRlc3R0ZXN0dGVzdHRlc3R0ZXN0dGVzdHRlc3Q=";
    "Test 1: Merge two capabilities with different permissions";
    cap1 = this:issue_capability(this, {'read, 'write}, 0, 0, test_key);
    cap2 = this:issue_capability(this, {'execute, 'delete}, 0, 0, test_key);
    merged = $root:merge_capability(cap1, cap2, test_key);
    typeof(merged) == TYPE_FLYWEIGHT || raise(E_ASSERT("Merged result should be a flyweight"));
    merged.delegate == this || raise(E_ASSERT("Merged delegate should match original"));
    "Test 2: Verify merged capability contains all permissions";
    {target, perms} = merged:challenge_for_with_key({'read, 'write, 'execute, 'delete}, test_key);
    target == this || raise(E_ASSERT("Merged capability should validate all permissions"));
    "Test 3: Merge with overlapping permissions";
    cap3 = this:issue_capability(this, {'read, 'write}, 0, 0, test_key);
    cap4 = this:issue_capability(this, {'write, 'execute}, 0, 0, test_key);
    merged2 = $root:merge_capability(cap3, cap4, test_key);
    {target2, perms2} = merged2:challenge_for_with_key({'read, 'write, 'execute}, test_key);
    target2 == this || raise(E_ASSERT("Merged overlapping should contain all unique permissions"));
    "Test 4: Merge with expiration - should take later expiration";
    future = time() + 3600;
    cap5 = this:issue_capability(this, {'read}, 0, 0, test_key);
    cap6 = this:issue_capability(this, {'write}, future, 0, test_key);
    merged3 = $root:merge_capability(cap5, cap6, test_key);
    claims = paseto_verify_local(merged3.token, test_key);
    maphaskey(claims, "exp") || raise(E_ASSERT("Merged should have expiration from cap6"));
    claims["exp"] == future || raise(E_ASSERT("Merged expiration should be later time"));
    "Test 5: Cannot merge capabilities for different targets";
    other_obj = this:create(true);
    cap7 = this:issue_capability(this, {'read}, 0, 0, test_key);
    cap8 = other_obj:issue_capability(other_obj, {'write}, 0, 0, test_key);
    merge_failed = false;
    try
      $root:merge_capability(cap7, cap8, test_key);
      merge_failed = true;
    except (E_INVARG)
    endtry
    !merge_failed || raise(E_ASSERT("Should not be able to merge caps for different targets"));
    return true;
  endmethod

  method test_grant_capability owner: ARCH_WIZARD
    "Test granting capabilities to players with auto-merge";
    test_key = "dGVzdHRlc3R0ZXN0dGVzdHRlc3R0ZXN0dGVzdHRlc3Q=";
    "Create test objects";
    test_area = #-1;
    test_player = #-1;
    test_room = #-1;
    try
      test_area = create($area);
      test_player = create($player);
      "Test 1: Grant initial capability";
      cap1 = $root:grant_capability(test_area, {'add_room}, test_player, 'area, test_key);
      typeof(cap1) == TYPE_FLYWEIGHT || raise(E_ASSERT("Should return capability flyweight"));
      cap1.delegate == test_area || raise(E_ASSERT("Capability should be for test_area"));
      "Test 2: Verify capability was stored in grants_area";
      typeof(test_player.grants_area) == TYPE_MAP || raise(E_ASSERT("grants_area should be a map"));
      maphaskey(test_player.grants_area, test_area) || raise(E_ASSERT("Should have grant for test_area"));
      stored_cap = test_player.grants_area[test_area];
      stored_cap == cap1 || raise(E_ASSERT("Stored capability should match returned one"));
      "Test 3: Grant additional capability - should auto-merge";
      cap2 = $root:grant_capability(test_area, {'create_passage}, test_player, 'area, test_key);
      typeof(cap2) == TYPE_FLYWEIGHT || raise(E_ASSERT("Second grant should return flyweight"));
      "Test 4: Verify merged capability has both permissions";
      merged_cap = test_player.grants_area[test_area];
      {target, perms} = merged_cap:challenge_for_with_key({'add_room, 'create_passage}, test_key);
      target == test_area || raise(E_ASSERT("Merged cap should grant both permissions"));
      "Test 5: find_capability_for retrieves the grant";
      found_cap = test_player:find_capability_for(test_area, 'area);
      found_cap == merged_cap || raise(E_ASSERT("find_capability_for should return stored grant"));
      "Test 6: Different category (room grants)";
      test_room = create($room);
      room_cap = $root:grant_capability(test_room, {'dig_from}, test_player, 'room, test_key);
      typeof(test_player.grants_room) == TYPE_MAP || raise(E_ASSERT("grants_room should be created"));
      maphaskey(test_player.grants_room, test_room) || raise(E_ASSERT("Should have grant for test_room"));
      found_room_cap = test_player:find_capability_for(test_room, 'room);
      found_room_cap == room_cap || raise(E_ASSERT("Should find room grant"));
    finally
      valid(test_room) && test_room:destroy();
      valid(test_player) && test_player:destroy();
      valid(test_area) && test_area:destroy();
    endtry
    return true;
  endmethod

  method test_presentation_defaults owner: ARCH_WIZARD
    "Test default root presentation and grammar helpers on a scratch child.";
    scratch = create($root);
    try
      scratch.name = "Root Test Object";
      scratch.aliases = {"root-test", "rto"};
      $test_utils:assert_eq(scratch:name(), "Root Test Object", "name() should return .name");
      $test_utils:assert_eq(scratch:display_name(), "Root Test Object", "display_name() should default to name()");
      $test_utils:assert_eq(scratch:aliases(), {"root-test", "rto"}, "aliases() should return .aliases");
      $test_utils:assert_false(scratch:is_actor(), "root descendants should not be actors by default");
      $test_utils:assert_false(scratch:is_plural(), "root descendants should be singular by default");
      $test_utils:assert_false(scratch:is_countable(), "root descendants should be uncountable by default");
      $test_utils:assert_true(scratch:is_proper_noun(), "root descendants should be proper nouns by default");
    finally
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod

  method test_mutators_and_help_defaults owner: ARCH_WIZARD
    "Test root metadata mutators and default help behavior.";
    scratch = create($root);
    new_owner = #-1;
    try
      scratch:set_name_aliases("Renamed Root Test", {"renamed-root", "rrt"});
      $test_utils:assert_eq(scratch.name, "Renamed Root Test", "set_name_aliases() should update name");
      $test_utils:assert_eq(scratch.aliases, {"renamed-root", "rrt"}, "set_name_aliases() should update aliases");
      scratch:set_description("Plain root test description.");
      $test_utils:assert_eq(scratch.description, "Plain root test description.", "set_description() should store plain strings");
      scratch:set_description("This is {n}.");
      $test_utils:assert_eq(scratch.description, "This is {n}.", "literal descriptions never infer substitutions");
      rejected = false;
      try
        scratch:set_description("Unsupported {+insert+}", 'djot_blocks);
      except (E_INVARG)
        rejected = true;
      endtry
      $test_utils:assert_true(rejected, "authored descriptions validate before saving");
      $test_utils:assert_eq(scratch.description, "This is {n}.", "a rejected edit preserves the source");
      new_owner = create($root);
      add_property(scratch, "root_test_owned_prop", "owned", {this.owner, "rc"});
      scratch:set_owner(new_owner);
      $test_utils:assert_eq(scratch.owner, new_owner, "set_owner() should update object owner");
      prop_info = property_info(scratch, "root_test_owned_prop");
      $test_utils:assert_eq(prop_info[1], new_owner, "set_owner() should retitle local c properties");
      $test_utils:assert_eq(scratch:object_help(), 0, "object_help() should default to no help");
    finally
      valid(new_owner) && recycle(new_owner);
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod

  method test_introspection_helpers owner: ARCH_WIZARD
    "Test root inheritance, property, verb, and branch introspection helpers.";
    scratch = create($root);
    child = create(scratch);
    grandchild = create(child);
    try
      all_verbs = child:all_verbs();
      $test_utils:assert_true("find_verb_definer" in all_verbs, "all_verbs() should include inherited root verbs");
      all_props = child:all_properties();
      $test_utils:assert_true('aliases in all_props, "all_properties() should include inherited root properties");
      $test_utils:assert_eq(child:find_verb_definer("display_name"), $root, "find_verb_definer() should find inherited root verbs");
      $test_utils:assert_eq(child:find_verb_definer("definitely_missing_root_test_verb"), #-1, "find_verb_definer() should return #-1 when absent");
      $test_utils:assert_true(child:check_property_exists("aliases"), "check_property_exists() should find inherited properties");
      $test_utils:assert_false(child:check_property_exists("definitely_missing_root_test_property"), "check_property_exists() should reject missing properties");
      branches = scratch:branches();
      $test_utils:assert_true(scratch in branches, "branches() should include the receiver when it has children");
      $test_utils:assert_true(child in branches, "branches() should include descendant branches");
      $test_utils:assert_false(grandchild in branches, "branches() should skip leaf descendants");
    finally
      valid(grandchild) && recycle(grandchild);
      valid(child) && recycle(child);
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod

  method test_examination_defaults owner: ARCH_WIZARD
    "Test default examination flyweight shape for root descendants.";
    scratch = create($root);
    try
      scratch.name = "Examined Root Test";
      scratch.aliases = {"examined-root"};
      scratch.description = "Examined description.";
      exam = scratch:examination();
      $test_utils:assert_type(exam, TYPE_FLYWEIGHT, "examination() should return a flyweight");
      $test_utils:assert_eq(exam.object_ref, scratch, "examination() should include object_ref");
      $test_utils:assert_eq(exam.name, "Examined Root Test", "examination() should include name");
      $test_utils:assert_eq(exam.aliases, {"examined-root"}, "examination() should include aliases");
      $test_utils:assert_eq(exam.description, "Examined description.", "examination() should include description");
      $test_utils:assert_eq(exam.owner, scratch.owner, "examination() should include owner");
      $test_utils:assert_eq(exam.parent, $root, "examination() should include parent");
      $test_utils:assert_eq(exam.location, scratch.location, "examination() should include location");
      $test_utils:assert_type(exam.verbs, TYPE_LIST, "examination() should include usable verbs list");
      $test_utils:assert_type(exam.contents, TYPE_LIST, "examination() should include contents list");
    finally
      valid(scratch) && recycle(scratch);
    endtry
    return true;
  endmethod
endobject
