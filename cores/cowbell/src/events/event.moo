object EVENT [
  import_export_id -> "event",
  import_export_hierarchy -> {"events"}
]
  name: "Event Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Captured event facts and a content template. Resolution produces one inert document for a viewer before any format is selected.";

  method "mk_*" owner: ARCH_WIZARD
    "Capture an action and its participant/location identities, with literal or structured content.";
    set_task_perms(caller_perms());
    const {actor, @content} = args;
    const present = typeof(actor) == TYPE_OBJ && valid(actor);
    (present || (typeof(actor) == TYPE_BOOL && !actor)) || raise(E_INVARG, "Invalid event actor");
    const actor_name = present ? actor:name() | "";
    const location = present ? actor.location | false;
    const action = verb[4..$];
    const blocks = $format:blocks(content);
    return <$event, .actor = actor, .actor_name = actor_name, .verb = action,
      .dobj = false, .iobj = false, .this_obj = false, .location = location,
      .timestamp = time(), .bindings = [], blocks>;
  endmethod

  method "with_dobj with_iobj with_this" owner: ARCH_WIZARD
    "Capture an object target on the immutable event value. No target is represented by false.";
    set_task_perms(caller_perms());
    const {value} = args;
    (typeof(value) == TYPE_OBJ || (typeof(value) == TYPE_BOOL && !value)) || raise(E_TYPE);
    let key = tosym(verb[6..$]);
    key == 'this && (key = 'this_obj);
    this.(key) = value;
    return this;
  endmethod

  method with_location owner: HACKER
    "Set the captured action location explicitly when an event describes movement.";
    const {location} = args;
    typeof(location) == TYPE_OBJ || raise(E_TYPE);
    this.location = location;
    return this;
  endmethod

  method "with_text_binding with_object_binding with_content_binding" owner: HACKER
    "Capture a named binding with an explicit kind. Text is never reinterpreted as markup.";
    const {name, value} = args;
    typeof(name) == TYPE_SYM || raise(E_TYPE);
    !(name in {'actor, 'dobj, 'iobj, 'this_obj, 'location, 'n, 'nc, 'd, 'dc,
      'i, 'ic, 'this, 't, 'tc, 'l, 'lc}) || raise(E_INVARG, "Reserved event binding");
    const kinds = ["with_text_binding" -> 'text, "with_object_binding" -> 'object,
      "with_content_binding" -> 'content];
    const kind = kinds[verb];
    if (kind == 'text)
      typeof(value) == TYPE_STR || raise(E_TYPE, "Text bindings require strings");
    elseif (kind == 'object)
      typeof(value) == TYPE_OBJ || raise(E_TYPE, "Object bindings require objects");
    else
      $format:inlines({value});
    endif
    let bindings = this.bindings;
    bindings[name] = ['kind -> kind, 'value -> value];
    this.bindings = bindings;
    return this;
  endmethod

  method resolve owner: ARCH_WIZARD
    "Resolve a canonical document under the caller's authority. False is a public, absent viewer.";
    set_task_perms(caller_perms());
    const {viewer} = args;
    return $document_resolver:resolve(flycontents(this), viewer, this:_facts());
  endmethod

  method render_for owner: ARCH_WIZARD
    "Resolve for one viewer and produce a shared document envelope for all output requests.";
    set_task_perms(caller_perms());
    const {viewer, requests} = args;
    return $document_resolver:render_for(flycontents(this), viewer, this:_facts(), requests);
  endmethod

  method _facts owner: HACKER
    "Select only captured event facts for resolution; metadata cannot supply its own principal.";
    this:validate() || raise(E_INVARG, "Malformed event");
    const fields = flyslots(this);
    let facts = [];
    for key in ({'actor, 'actor_name, 'verb, 'dobj, 'iobj, 'this_obj, 'location, 'timestamp, 'bindings})
      facts[key] = fields[key];
    endfor
    return facts;
  endmethod

  method validate owner: HACKER
    "Check required captured fields without invoking content code.";
    typeof(this) == TYPE_FLYWEIGHT && this.delegate == $event || return false;
    const fields = flyslots(this);
    for key in ({'actor, 'actor_name, 'verb, 'dobj, 'iobj, 'timestamp, 'this_obj, 'location, 'bindings})
      maphaskey(fields, key) || return false;
    endfor
    return typeof(fields['bindings]) == TYPE_MAP;
  endmethod

  method with_metadata owner: ARCH_WIZARD
    "Set arbitrary metadata on the event.";
    {key, value} = args;
    this.(key) = value;
    return this;
  endmethod

  method audience owner: HACKER
    "Return the audience classification stored on this event.";
    return `this.audience ! E_PROPNF => 'narrative';
  endmethod

  method with_audience owner: ARCH_WIZARD
    "Attach an audience classification to the event.";
    {audience} = args;
    this.audience = audience;
    return this;
  endmethod

  method presentation_hint owner: HACKER
    "Return the presentation hint stored on this event.";
    return `this.presentation_hint ! E_PROPNF => false';
  endmethod

  method with_presentation_hint owner: HACKER
    "Attach a presentation hint to the event.";
    {hint} = args;
    this.presentation_hint = hint;
    return this;
  endmethod

  method with_group owner: HACKER
    "Attach a group_id for client-side message bundling.";
    "Call with (prefix) for unique per-event grouping, or (prefix, target) for stable grouping.";
    {prefix, ?target = $nothing} = args;
    if (target != $nothing && typeof(target) == TYPE_OBJ && valid(target))
      group_id = tostr(prefix) + "_" + tostr(target);
    else
      group_id = tostr(prefix) + "_" + uuid();
    endif
    this.group_id = group_id;
    return this;
  endmethod

  method with_tts owner: HACKER
    "Attach TTS-friendly text for screen readers.";
    {text} = args;
    this.tts_text = text;
    return this;
  endmethod

  method ensure_audience owner: ARCH_WIZARD
    "Ensure event has the specified audience, return event.";
    {audience} = args;
    `this.audience ! E_PROPNF => 0' == audience && return this;
    this.audience = audience;
    return this;
  endmethod

  method with_rewritable owner: ARCH_WIZARD
    "Mark event as rewritable. Caller provides ID, optional TTL (default 60s), optional fallback.";
    {rewrite_id, ?ttl = 60, ?fallback = 0} = args;
    this.rewritable_id = rewrite_id;
    this.rewritable_owner = this.actor;
    this.rewritable_ttl = ttl;
    fallback && (this.rewritable_fallback = fallback);
    return this;
  endmethod

  verb as_inset (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Mark event for inset presentation.";
    this.presentation_hint = 'inset;
    return this;
  endverb

endobject
