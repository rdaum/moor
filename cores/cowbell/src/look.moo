object LOOK [
  import_export_id -> "look"
]
  name: "Object Look"
  parent: ROOT
  owner: HACKER

  override description (owner: HACKER, flags: "rc") = "Look inputs capture one viewer's document and references before narrative or panel layout.";

  method mk owner: ARCH_WIZARD
    "Collect an object's authored description and visible contents for later viewer capture.";
    set_task_perms(caller_perms());
    const {what, @contents} = args;
    return <this, .what = what, .title = what:name(), .description = what:description(),
      .exit_options = {}, .exit_revision = 0, .ambient_passages = {}, .actions = {}, contents>;
  endmethod

  method actor_idle_status owner: HACKER
    "Describe a player's current connection state; NPCs have no idle status.";
    const {who} = args;
    is_player(who) || return "";
    who in connected_players() || return "deeply asleep";
    const idle = idle_seconds(who);
    typeof(idle) == TYPE_ERR && return "";
    idle < 60 && return "awake";
    idle < 180 && return "dozing";
    idle < 600 && return "idle";
    idle < 1800 && return "out on his feet";
    return "sleeping";
  endmethod

  method capture owner: ARCH_WIZARD
    "Capture labels, references, and resolved description once under the caller's authority.";
    "The returned LookView has no deferred content callbacks. Layout methods use only its values.";
    set_task_perms(caller_perms());
    const {viewer} = args;
    const event = $event:mk_look(viewer):with_dobj(this.what)
      :with_presentation_hint('inset):with_group('look, this.what)
      :with_metadata('collapse_title, this.title);
    let description = typeof(this.description) == TYPE_LIST ? this.description | {this.description};
    let actors = {};
    let things = {};
    let subjects = [];
    for item in (flycontents(this))
      if (!valid(item) || item == viewer)
        continue;
      endif
      const integrated = `item:integrate_description() ! E_VERBNF => false';
      const has_integration = typeof(integrated) == TYPE_FLYWEIGHT
        || (typeof(integrated) in {TYPE_STR, TYPE_LIST} && length(integrated));
      if (has_integration)
        const content = typeof(integrated) == TYPE_STR && !integrated:ends_with(".")
          ? integrated + "." | integrated;
        description = {@description, "  ", content};
      endif
      const actor = item:is_actor();
      const status = actor ? $look:actor_idle_status(item) | "";
      const {name, named} = $document_resolver:capture_subject(item, viewer, 'listing, subjects);
      subjects = named;
      const label = length(status) && status != "awake" ? name + " (" + status + ")" | name;
      const descriptor = is_anonymous(item) ? [] | ["kind" -> "object",
        "ref" -> $url_utils:to_curie_str(item), "objectKind" -> item:reference_kind()];
      let entry = $look:_entry(label, descriptor);
      entry['object] = item;
      entry['status] = status;
      entry['bare] = $look:_entry(name, descriptor);
      entry['integrated] = has_integration;
      if (actor)
        const {grammar, captured} = $document_resolver:capture_subject(item, viewer, 'grammar, subjects);
        subjects = captured;
        entry['plural] = grammar['number] == 'plural;
        actors = {@actors, entry};
      else
        things = {@things, entry};
      endif
    endfor
    const ambient = `this.ambient_passages ! E_PROPNF => {}';
    let exits = {};
    let exit_descriptors = [];
    for option in (`this.exit_options ! E_PROPNF => {}')
      const descriptor = option['descriptor];
      exit_descriptors[option['label]] = descriptor;
      let entry = $look:_entry(option['caption], descriptor);
      entry['ambient] = option['ambient];
      exits = {@exits, entry};
    endfor
    description = {@description, @$look:_ambient(ambient, exit_descriptors, event, viewer)};
    const source = toflyweight($event, flyslots(event), $format:blocks(description));
    const resolved = source:resolve(viewer);
    let actions = {};
    for action in (`this.actions ! E_PROPNF => {}')
      const {command, label} = action;
      actions = {@actions, $look:_entry(label, ["kind" -> "command", "command" -> command])};
    endfor
    const thumbnail = this.what:thumbnail();
    const title_descriptor = is_anonymous(this.what) ? [] | ["kind" -> "object",
      "ref" -> $url_utils:to_curie_str(this.what), "objectKind" -> this.what:reference_kind()];
    const title = $look:_entry(this.title, title_descriptor)['content];
    return ['room_ref -> this.what, 'title -> title,
      'title_text -> this.title, 'description -> resolved, 'exits -> exits,
      'actors -> actors, 'things -> things, 'actions -> actions, 'event -> event,
      'viewer -> viewer, 'thumbnail -> thumbnail,
      'exit_revision -> `this.exit_revision ! E_PROPNF => 0'];
  endmethod

  method _entry owner: HACKER
    "Pair a literal label with a captured interaction descriptor and canonical inline nodes.";
    const {label, descriptor} = args;
    const text = $format:literal_inline(label);
    const content = length(descriptor) ? {$document_node:mk('reference, text,
      ['descriptor -> $document_schema:_descriptor_from_wire(descriptor)])} | text;
    return ['label -> label, 'descriptor -> descriptor, 'content -> content];
  endmethod

  method _ambient owner: ARCH_WIZARD
    "Resolve each passage's inline content with its own bindings, then assemble captured prose.";
    set_task_perms(caller_perms());
    const {passages, descriptors, event, viewer} = args;
    let sentences = {};
    let fragments = {};
    for passage in (passages)
      const {source, style, ?direction = "", ?destination = #-1} = passage;
      let context = event:with_this(event.dobj):with_location(event.dobj)
        :with_object_binding('from_room, event.dobj):with_object_binding('to_room, destination)
        :with_text_binding('direction, direction):with_text_binding('passage, direction);
      const template = toflyweight($event, flyslots(context), $format:blocks({source}));
      const resolved = template:resolve(viewer);
      if (!length(flycontents(resolved)))
        continue;
      endif
      let content = flycontents(flycontents(resolved)[1]);
      if (maphaskey(descriptors, direction) && length(descriptors[direction])
          && !$look:_has_inline_link(content))
        content = {$document_node:mk('reference, content,
          ['descriptor -> $document_schema:_descriptor_from_wire(descriptors[direction])])};
      endif
      if (style == 'fragment)
        fragments = {@fragments, ['content -> content]};
      else
        length(sentences) && (sentences = {@sentences, @$format:literal_inline(" ")});
        sentences = {@sentences, @content};
      endif
    endfor
    if (length(fragments))
      sentences = {@sentences, @$format:literal_inline(length(sentences) ? " You see " | "You see "),
        @$look:_join(fragments), @$format:literal_inline(".")};
    endif
    return length(sentences) ? {$document_node:mk('paragraph, sentences)} | {};
  endmethod

  method _has_inline_link owner: HACKER
    "Avoid nesting a generated exit reference around authored links or object references.";
    const {nodes} = args;
    for node in (nodes)
      node.kind in {'reference, 'link} && return true;
      $look:_has_inline_link(flycontents(node)) && return true;
    endfor
    return false;
  endmethod

  method _join owner: HACKER
    "Join captured entry labels with commas and a final conjunction.";
    const {entries} = args;
    let content = {};
    for i in [1..length(entries)]
      if (i > 1)
        const separator = i == length(entries) ? " and " | ", ";
        content = {@content, @$format:literal_inline(separator)};
      endif
      content = {@content, @entries[i]['content]};
    endfor
    return content;
  endmethod

  method narrative owner: HACKER
    "Assemble a narrative event from an inert LookView. No object labels or content are reread.";
    const {view} = args;
    let blocks = {$document_node:mk('heading, view['title], ['level -> 3]),
      @flycontents(view['description])};
    const exits = view['exits];
    if (length(exits))
      const prefix = length(exits) == 1 ? "An exit leads out " | "Exits lead out ";
      blocks = {@blocks, $document_node:mk('paragraph, {
        @$format:literal_inline(prefix), @$look:_join(exits), @$format:literal_inline(".")})};
    endif
    if (length(view['actions]))
      blocks = {@blocks, $look:_list(view['actions])};
    endif
    const things = {thing for thing in (view['things]) if (!thing['integrated])};
    if (length(things))
      blocks = {@blocks, $document_node:mk('paragraph, {
        @$format:literal_inline("You see "), @$look:_join(things),
        @$format:literal_inline(" here.")})};
    endif
    for sleeping in ({false, true})
      const actors = {actor for actor in (view['actors]) if (
        !actor['integrated] && ((actor['status] == "deeply asleep") == sleeping))};
      if (length(actors))
        const ending = ((length(actors) > 1 || actors[1]['plural]) ? " are" | " is")
          + (sleeping ? " deeply asleep." | " here.");
        "Sleeping is already expressed by the sentence, so omit the status suffix.";
        const labels = sleeping ? {sleeping_actor['bare] for sleeping_actor in (actors)} | actors;
        blocks = {@blocks, $document_node:mk('paragraph, {
          @$document_resolver:_capitalize($look:_join(labels))[1],
          @$format:literal_inline(ending)})};
      endif
    endfor
    let event = toflyweight($event, flyslots(view['event]), blocks);
    if (typeof(view['thumbnail]) == TYPE_LIST && length(view['thumbnail]) == 2)
      event = event:with_metadata('thumbnail, view['thumbnail]);
    endif
    return event;
  endmethod

  method _list owner: HACKER
    "Build a canonical reference list from captured entries.";
    const {entries} = args;
    return $document_node:mk('list, {['blocks -> {
      $document_node:mk('paragraph, entry['content])}] for entry in (entries)}, ['style -> 'bullet]);
  endmethod

  method panel owner: HACKER
    "Render a panel from the captured description and entry groups. Its title stays separate.";
    const {view, ?format = 'text_html} = args;
    let blocks = flycontents(view['description]);
    for section in ({{"Exits", view['exits]}, {"Things", {@view['actions], @view['things]}},
        {"People", view['actors]}})
      const {label, entries} = section;
      if (length(entries))
        blocks = {@blocks, $document_node:mk('heading, $format:literal_inline(label),
          ['level -> 4]), $look:_list(entries)};
      endif
    endfor
    const result = $document:render($document:mk(blocks), format);
    return ["room" -> view['room_ref], "title" -> view['title_text],
      "content" -> result['body], "content_type" -> strsub(tostr(format), "_", "/", 1),
      "annotations" -> result['annotations]];
  endmethod
endobject
