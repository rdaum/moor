object FORMAT_DEFLIST [
  import_export_id -> "FORMAT_DEFLIST",
  import_export_hierarchy -> {"format"}
]
  name: "Definition List Flyweight Delegate"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Flyweight delegate for definition lists (key-value pairs) in events.";

  method mk owner: ARCH_WIZARD
    "Construct ordered definitions from term/content pairs. Definitions can contain several blocks.";
    const {pairs} = args;
    typeof(pairs) == TYPE_LIST || raise(E_TYPE);
    let items = {};
    for pair in (pairs)
      const {term, definition} = pair;
      items = {@items, ['term -> $format:inlines({term}),
        'definition -> $format:blocks({definition})]};
    endfor
    return $document_template:mk('definition_list, items);
  endmethod
endobject
