object HELP [
  import_export_id -> "help",
  import_export_hierarchy -> {"help"}
]
  name: "Help Topic Flyweight Delegate"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Flyweight delegate for help topics. Creates structured help entries that can be rendered for humans or machines.";

  method mk owner: ARCH_WIZARD
    "Create a help topic flyweight.";
    "Args: (name, summary, content, ?aliases, ?category, ?see_also)";
    {name, summary, content, ?aliases = {}, ?category = 'general, ?see_also = {}} = args;
    return <this, .name = name, .summary = summary, .content = content, .aliases = aliases, .category = category, .see_also = see_also>;
  endmethod

  method from_provider owner: ARCH_WIZARD
    "Retain the exact help provider for annotations in topic listings.";
    const {provider} = args;
    this.provider = provider;
    return this;
  endmethod

  method matches owner: ARCH_WIZARD
    "Check if this help topic matches a search query (supports prefix matching).";
    {query} = args;
    "Exact match on name";
    this.name == query && return true;
    "Prefix match on name";
    index(this.name, query) == 1 && return true;
    "Check aliases";
    for alias in (this.aliases)
      alias == query && return true;
      index(alias, query) == 1 && return true;
    endfor
    return false;
  endmethod

  method render_prose owner: ARCH_WIZARD
    "Return authored prose or programmatic blocks, followed by related-topic references.";
    const viewer = player;
    const body = typeof(this.content) == TYPE_STR
      ? $help_utils:annotate_prose(this.content, viewer) | this.content;
    let lines = {this.summary, "", body};
    if (length(this.see_also))
      const topics = viewer:_collect_help_topics();
      let references = {};
      for name in (this.see_also)
        let reference = name;
        for topic in (topics)
          if (topic.name == name || name in topic.aliases)
            reference = $format.annotation:help(topic.provider, topic.name, name);
            break;
          endif
        endfor
        references = {@references, reference};
      endfor
      lines = {@lines, "", "See also:", $format.list:mk(references, false)};
    endif
    return lines;
  endmethod

  method render_structured owner: ARCH_WIZARD
    "Return structured data for agents/LLMs.";
    return ['name -> this.name, 'aliases -> this.aliases, 'category -> this.category, 'summary -> this.summary, 'content -> this.content, 'see_also -> this.see_also];
  endmethod
endobject
