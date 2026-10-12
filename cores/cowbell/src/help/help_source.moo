object HELP_SOURCE [
  import_export_id -> "help_source",
  import_export_hierarchy -> {"help"}
]
  name: "Help Source Prototype"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  property topic_order (owner: ARCH_WIZARD, flags: "rc") = {};
  property content_kind (owner: ARCH_WIZARD, flags: "rc") = 'literal;

  method help_topics owner: ARCH_WIZARD
    "Generic property-backed help topic provider.";
    "Reads ordered topic properties as either $help flyweights or tuple data.";
    {for_player, ?topic = ""} = args;
    props = properties(this);
    order = this.topic_order;
    if (typeof(order) != TYPE_LIST)
      order = {};
    endif
    seen = [];
    names = {};
    for p in (order)
      if (typeof(p) == TYPE_SYM && p in props)
        names = {@names, p};
        seen[p] = 1;
      endif
    endfor
    for p in (props)
      if (typeof(p) == TYPE_SYM && p:starts_with('topic_) && !maphaskey(seen, p))
        names = {@names, p};
      endif
    endfor
    my_topics = {};
    for prop in (names)
      data = this.(prop);
      if (typeof(data) == TYPE_FLYWEIGHT)
        t = data;
      elseif (typeof(data) == TYPE_LIST && length(data) >= 3)
        {name, summary, content, ?aliases = {}, ?category = "general", ?see_also = {}} = data;
        if (typeof(name) != TYPE_STR || !name)
          continue;
        endif
        if (typeof(summary) != TYPE_STR)
          summary = tostr(summary);
        endif
        if (typeof(content) != TYPE_STR)
          content = tostr(content);
        endif
        typeof(aliases) == TYPE_LIST || (aliases = {});
        typeof(see_also) == TYPE_LIST || (see_also = {});
        if (typeof(category) == TYPE_STR)
          cat = tosym(category);
        else
          cat = category;
        endif
        t = $help:mk(name, summary, content, aliases, cat, see_also, this.content_kind);
      else
        continue;
      endif
      if (topic == "")
        my_topics = {@my_topics, t};
      elseif (t:matches(topic))
        return t;
      endif
    endfor
    return topic == "" ? my_topics | 0;
  endmethod

  method test_help_source_symbol_topic_discovery owner: ARCH_WIZARD
    "Property-backed help should use symbol keys while keeping fallback discovery under topic_*.";
    src = this:create(true);
    add_property(src, 'metadata, {"metadata", "Not help", "This should not become a topic."}, {src.owner, "r"});
    add_property(src, 'ordered_metadata, {"ordered", "Ordered help", "Explicitly ordered non-topic symbol."}, {src.owner, "r"});
    add_property(src, 'topic_short, {"short", "Short tuple", "Tuple with default aliases/category/see_also."}, {src.owner, "r"});
    add_property(src, 'topic_full, {"full", "Full tuple", "Tuple with all fields.", {"complete"}, "testing", {"short"}}, {src.owner, "r"});
    src.topic_order = {'ordered_metadata};
    ordered = src:help_topics($test_player, "ordered");
    $test_utils:assert_type(ordered, TYPE_FLYWEIGHT, "topic_order should allow explicit non-topic symbol properties");
    $test_utils:assert_eq(ordered.name, "ordered", "ordered topic name");
    metadata = src:help_topics($test_player, "metadata");
    $test_utils:assert_eq(metadata, 0, "fallback discovery should ignore non-topic symbol properties");
    short = src:help_topics($test_player, "short");
    $test_utils:assert_type(short, TYPE_FLYWEIGHT, "fallback topic_ symbol property should resolve");
    $test_utils:assert_eq(short.name, "short", "short tuple topic name");
    $test_utils:assert_eq(short.aliases, {}, "short tuple should default aliases");
    $test_utils:assert_eq(short.category, 'general, "short tuple should default category");
    $test_utils:assert_eq(short.see_also, {}, "short tuple should default see_also");
    full = src:help_topics($test_player, "complete");
    $test_utils:assert_type(full, TYPE_FLYWEIGHT, "aliases should match full tuple topic");
    $test_utils:assert_eq(full.name, "full", "full tuple topic name");
    return true;
  endmethod
endobject
