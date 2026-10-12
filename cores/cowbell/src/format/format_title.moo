object FORMAT_TITLE [
  import_export_id -> "format_title",
  import_export_hierarchy -> {"format"}
]
  name: "Title Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for title/heading content in events.";

  method mk owner: HACKER
    "Construct a heading with literal inline content. The default level is three.";
    const {content, ?level = 3} = args;
    typeof(level) == TYPE_INT || raise(E_TYPE);
    level >= 1 && level <= 6 || raise(E_INVARG, "Heading level must be one through six");
    return $document_template:mk('heading, $format:inlines({content}), ['level -> level]);
  endmethod
endobject
