object FORMAT_LINK [
  import_export_id -> "format_link",
  import_export_hierarchy -> {"format"}
]
  name: "Link Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "External URLs and inline formatted content.";

  method external owner: HACKER
    "Construct an external link. Its default label is the literal URL.";
    const {url, ?label = url} = args;
    typeof(url) == TYPE_STR || raise(E_TYPE);
    return $document_template:mk('link, $format:inlines({label}), ['url -> url]);
  endmethod

  method inline owner: HACKER
    "Construct one inline sequence from literal text and document content.";
    const {parts} = args;
    return $document_template:mk('inline, $format:inlines(parts));
  endmethod

endobject
