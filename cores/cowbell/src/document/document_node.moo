object DOCUMENT_NODE [
  import_export_id -> "document_node",
  import_export_hierarchy -> {"document"}
]
  name: "Document Node"
  parent: ROOT
  owner: HACKER
  readable: true

  method mk owner: HACKER
    "Construct a node with explicit kind, ordered contents, and symbol-keyed scalar fields.";
    "List items, definitions, table rows, cells, and columns are symbol-keyed records.";
    const {kind, ?contents = {}, ?fields = []} = args;
    typeof(kind) == TYPE_SYM || raise(E_TYPE);
    typeof(contents) == TYPE_LIST || raise(E_TYPE);
    typeof(fields) == TYPE_MAP || raise(E_TYPE);
    !maphaskey(fields, 'kind) || raise(E_INVARG, "kind is a separate constructor argument");
    for key in (mapkeys(fields))
      typeof(key) == TYPE_SYM || raise(E_TYPE, "Document node fields require symbol keys");
    endfor
    let slots = fields;
    slots['kind] = kind;
    return toflyweight($document_node, slots, contents);
  endmethod
endobject
