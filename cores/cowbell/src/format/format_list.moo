object FORMAT_LIST [
  import_export_id -> "format_list",
  import_export_hierarchy -> {"format"}
]
  name: "List Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for list content in events.";

  method mk owner: HACKER
    "Construct a list. Each item can contain inline content or an explicit block group.";
    const {items, ?ordered = false, ?start = 1} = args;
    typeof(items) == TYPE_LIST && typeof(ordered) == TYPE_BOOL || raise(E_TYPE);
    const records = {['blocks -> $format:blocks({item})] for item in (items)};
    let fields = ['style -> ordered ? 'numbered | 'bullet];
    ordered && (fields['start] = start);
    return $document_template:mk('list, records, fields);
  endmethod

  method actions owner: HACKER
    "Construct a list of action references. Layout belongs to the renderer.";
    const {items} = args;
    return $format.list:mk(items);
  endmethod
endobject
