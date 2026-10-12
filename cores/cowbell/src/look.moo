object LOOK [
  import_export_id -> "look"
]
  name: "Object 'look' Flyweight Delegate"
  parent: ROOT
  owner: HACKER

  override description (owner: HACKER, flags: "rc") = "The $look flyweight delegate holds the attributes involved in looking at an object, and can be transformed into output events. It always has mandatory 'title and 'description slots, and then optional contents which are a series of integration descriptions.";

  method mk owner: HACKER
    {what, @contents} = args;
    return <this, .what = what, .title = what:name(), .description = what:description(), .exits = {}, .ambient_passages = {}, .actions = {}, {@contents}>;
  endmethod

  method actor_idle_status owner: HACKER
    "Get a descriptive idle status for an actor (player or NPC)";
    {who} = args;
    "NPCs don't have connection state - no status";
    is_player(who) || return "";
    "Players who aren't connected are sleeping";
    who in connected_players() || return "deeply asleep";
    if (typeof(idle = idle_seconds(who)) == TYPE_ERR)
      return "";
    endif
    idle < 60 && return "awake";
    idle < 180 && return "dozing";
    idle < 600 && return "idle";
    idle < 1800 && return "out on his feet";
    return "sleeping";
  endmethod

  method into_event owner: HACKER
    "Three lines -- title, description, contents.";
    "Description is the item description but is also appended to with integrations. Objects with an :integrate_description verb are put there.";
    "The remainder go into the contents block, separated by type: actors vs things.";
    title = $format.title:mk($sub:dc());
    "Normalize base description so lists with $sub flyweights remain intact";
    description = this.description;
    desc_content = {};
    if (typeof(description) == TYPE_LIST)
      desc_content = description;
    elseif (typeof(description) in {TYPE_STR, TYPE_FLYWEIGHT})
      desc_content = {description};
    else
      desc_content = {tostr(description)};
    endif
    integrated_contents = {};
    things = {};
    actors = {};
    deeply_asleep = {};
    flyweight_contents = flycontents(this);
    for o in (flyweight_contents)
      if (o == player)
        continue;
      endif
      integrated_description = `o:integrate_description() ! E_VERBNF => false';
      if (typeof(integrated_description) == TYPE_FLYWEIGHT
          || (typeof(integrated_description) in {TYPE_STR, TYPE_LIST} && length(integrated_description)))
        integrated_contents = {@integrated_contents, integrated_description};
      elseif (o:is_actor())
        "Collect actor with idle status for inspect links";
        status = this:actor_idle_status(o);
        if (status == "deeply asleep")
          deeply_asleep = {@deeply_asleep, {o, ""}};
        elseif (status && status != "awake")
          actors = {@actors, {o, status}};
        else
          actors = {@actors, {o, ""}};
        endif
      else
        things = {@things, o};
      endif
    endfor
    "Combine integrated object descriptions and ambient passages into the main paragraph";
    ambient_passages = `this.ambient_passages ! E_PROPNF => {}';
    if (length(integrated_contents))
      for ic in (integrated_contents)
        if (typeof(ic) == TYPE_STR && !ic:ends_with("."))
          ic = ic + ".";
        endif
        desc_content = {@desc_content, "  ", ic};
      endfor
    endif
    if (length(ambient_passages))
      "Process ambient passages based on prose_style";
      fragment_passages = {};
      for ap in (ambient_passages)
        "Handle formats: string, {desc, style}, or {desc, style, label}";
        label = "";
        if (typeof(ap) == TYPE_LIST && length(ap) >= 3)
          {description, prose_style, label} = ap;
        elseif (typeof(ap) == TYPE_LIST && length(ap) >= 2)
          {description, prose_style} = ap;
        else
          description = ap;
          prose_style = 'fragment;
        endif
        if (prose_style == 'sentence)
          "Complete sentence - linkify direction if label provided";
          if (label)
            formatted = $format.link:linkify_direction(description, label, this.what);
            "Capitalize the result";
            if (typeof(formatted) == TYPE_STR)
              formatted = formatted:capitalize();
              if (!formatted:ends_with(".") && !formatted:ends_with("!") && !formatted:ends_with("?"))
                formatted = formatted + ".";
              endif
            endif
          else
            formatted = description:capitalize();
            if (typeof(formatted) == TYPE_STR && !formatted:ends_with(".") && !formatted:ends_with("!") && !formatted:ends_with("?"))
              formatted = formatted + ".";
            endif
          endif
          desc_content = {@desc_content, "  ", formatted};
        else
          "Fragment - strip trailing punctuation since we wrap in 'You see X.'";
          if (typeof(description) == TYPE_STR)
            while (length(description) > 0 && (description:ends_with(".") || description:ends_with("!") || description:ends_with("?")))
              description = description[1..length(description) - 1];
            endwhile
          endif
          "Linkify with lowercase if label provided, collect for 'You see X' treatment";
          if (label)
            linkified = $format.link:linkify_direction(description, label, this.what, true);
            fragment_passages = {@fragment_passages, linkified};
          else
            fragment_passages = {@fragment_passages, description:initial_lowercase()};
          endif
        endif
      endfor
      "Combine fragment passages with 'You see' wrapper";
      if (length(fragment_passages))
        "Build paragraph content: 'You see X, Y and Z.'";
        parts = {"  You see "};
        for i in [1..length(fragment_passages)]
          frag = fragment_passages[i];
          if (i > 1 && i == length(fragment_passages))
            parts = {@parts, " and "};
          elseif (i > 1)
            parts = {@parts, ", "};
          endif
          parts = {@parts, frag};
        endfor
        parts = {@parts, "."};
        desc_content = {@desc_content, $format.paragraph:inline(parts)};
      endif
    endif
    "Retain explicit blocks in programmatic descriptions";
    block_elements = {title, @$format:blocks(desc_content)};
    "Add exits if present (with command links)";
    exits = `this.exits ! E_PROPNF => {}';
    if (length(exits) > 0)
      block_elements = {@block_elements, this:format_exits(exits)};
    endif
    "Add actions if present (interactive object commands)";
    actions = `this.actions ! E_PROPNF => {}';
    if (length(actions) > 0)
      block_elements = {@block_elements, this:format_actions(actions)};
    endif
    if (length(things))
      block_elements = {@block_elements, this:format_things(things)};
    endif
    if (length(actors))
      block_elements = {@block_elements, this:format_actors(actors)};
    endif
    if (length(deeply_asleep))
      block_elements = {@block_elements, this:format_sleeping(deeply_asleep)};
    endif
    b = $format.block:mk(@block_elements);
    event = $event:mk_look(player, b):with_dobj(this.what):with_metadata('preferred_content_types, {'text_html, 'text_plain}):with_presentation_hint('inset):with_group('look, this.what):with_metadata('collapse_title, this.what:name());
    "Add thumbnail if the target has one";
    if (respond_to(this.what, 'thumbnail))
      pic = `this.what:thumbnail() ! ANY => false';
      if (pic && typeof(pic) == TYPE_LIST && length(pic) == 2)
        event = event:with_metadata('thumbnail, pic);
      endif
    endif
    return event;
  endmethod

  method validate owner: HACKER
    typeof(this) != TYPE_FLYWEIGHT && return false;
    try
      return this.what && this.title && this.description;
    except (E_PROPNF)
      return false;
    endtry
  endmethod

  method format_exits owner: ARCH_WIZARD
    "Create a paragraph for exit links.";
    "Args: list of exit direction strings.";
    {exits} = args;
    typeof(exits) == TYPE_LIST || raise(E_TYPE, "Exits must be a list");
    length(exits) == 0 && return "";
    "Build parts list with links separated by commas";
    "Exit links retain the originating room and registered passage.";
    parts = {};
    if (length(exits) == 1)
      parts = {"An exit leads out ", $format.annotation:exit(this.what, exits[1]), "."};
    else
      parts = {"Exits lead out "};
      for i in [1..length(exits)]
        if (i > 1 && i == length(exits))
          parts = {@parts, " and "};
        elseif (i > 1)
          parts = {@parts, ", "};
        endif
        parts = {@parts, $format.annotation:exit(this.what, exits[i])};
      endfor
      parts = {@parts, "."};
    endif
    return $format.paragraph:mk(parts);
  endmethod

  method format_things owner: ARCH_WIZARD
    "Create a paragraph for 'You see X, Y and Z here.' with inspect links.";
    "Args: list of objects.";
    {objects} = args;
    typeof(objects) == TYPE_LIST || raise(E_TYPE, "Objects must be a list");
    length(objects) == 0 && return "";
    parts = {"You see "};
    for i in [1..length(objects)]
      if (i > 1 && i == length(objects))
        parts = {@parts, " and "};
      elseif (i > 1)
        parts = {@parts, ", "};
      endif
      item = objects[i];
      label = `item:display_name() ! E_VERBNF => item.name';
      parts = {@parts, $format.annotation:object(item, label)};
    endfor
    parts = {@parts, " here."};
    return $format.paragraph:mk(parts);
  endmethod

  method format_actors owner: ARCH_WIZARD
    "Create a paragraph for 'X, Y and Z are here.' with inspect links.";
    "Args: list of {actor, status} pairs where status can be empty string.";
    {actor_data} = args;
    typeof(actor_data) == TYPE_LIST || raise(E_TYPE, "Actor data must be a list");
    length(actor_data) == 0 && return "";
    parts = {};
    for i in [1..length(actor_data)]
      if (i > 1 && i == length(actor_data))
        parts = {@parts, " and "};
      elseif (i > 1)
        parts = {@parts, ", "};
      endif
      {actor, status} = actor_data[i];
      actor_name = `actor:name() ! E_VERBNF => actor.name';
      label = status && status != "" ? actor_name + " (" + status + ")" | actor_name;
      parts = {@parts, $format.annotation:object(actor, label)};
    endfor
    verb_form = length(actor_data) == 1 ? " is" | " are";
    parts = {@parts, verb_form, " here."};
    return $format.paragraph:mk(parts);
  endmethod

  method format_sleeping owner: ARCH_WIZARD
    "Create a paragraph for 'X, Y and Z are deeply asleep.' with inspect links.";
    "Args: list of {actor, status} pairs (status ignored for sleeping).";
    {actor_data} = args;
    typeof(actor_data) == TYPE_LIST || raise(E_TYPE, "Actor data must be a list");
    length(actor_data) == 0 && return "";
    parts = {};
    for i in [1..length(actor_data)]
      if (i > 1 && i == length(actor_data))
        parts = {@parts, " and "};
      elseif (i > 1)
        parts = {@parts, ", "};
      endif
      {actor, status} = actor_data[i];
      actor_name = `actor:name() ! E_VERBNF => actor.name';
      parts = {@parts, $format.annotation:object(actor, actor_name)};
    endfor
    verb_form = length(actor_data) == 1 ? " is" | " are";
    parts = {@parts, verb_form, " deeply asleep."};
    return $format.paragraph:mk(parts);
  endmethod

  method format_actions owner: ARCH_WIZARD
    "Create a paragraph for action links from interactive objects.";
    "Args: list of {command, label} tuples.";
    {actions} = args;
    typeof(actions) == TYPE_LIST || raise(E_TYPE, "Actions must be a list");
    length(actions) == 0 && return "";
    "Build parts list with links";
    parts = {};
    for i in [1..length(actions)]
      {cmd, label} = actions[i];
      if (i > 1)
        parts = {@parts, " "};
      endif
      parts = {@parts, $format.annotation:command(cmd, label)};
    endfor
    return $format.paragraph:mk(parts);
  endmethod

  method into_presentation owner: ARCH_WIZARD
    "Return a panel payload with its heading separated before serialization.";
    const {?content_type = 'text_html} = args;
    const title = this.what:name();
    const document = this:into_event():resolve(player);
    let blocks = flycontents(document);
    if (length(blocks) && blocks[1].kind == 'heading)
      blocks = blocks[2..$];
    endif
    const result = $document:render($document:mk(blocks), content_type);
    return ["title" -> title, "content" -> result['body],
      "content_type" -> tostr(content_type), "annotations" -> result['annotations]];
  endmethod
endobject
