object DOCUMENT_TEMPLATE [
  import_export_id -> "document_template",
  import_export_hierarchy -> {"document"}
]
  name: "Document Template"
  parent: ROOT
  owner: HACKER
  readable: true

  method mk owner: HACKER
    "Construct unresolved content. Its fields follow the node schema until resolution.";
    const {kind, ?contents = {}, ?fields = []} = args;
    typeof(kind) == TYPE_SYM && typeof(contents) == TYPE_LIST
      && typeof(fields) == TYPE_MAP || raise(E_TYPE);
    !maphaskey(fields, 'kind) || raise(E_INVARG, "kind is a separate argument");
    let slots = fields;
    slots['kind] = kind;
    return toflyweight($document_template, slots, contents);
  endmethod

  method with_action owner: HACKER
    "Attach an interaction-protocol action hint to a reference. It carries no authority.";
    const {action} = args;
    this.kind == 'reference || raise(E_INVARG, "Only references have actions");
    typeof(action) == TYPE_MAP || raise(E_TYPE);
    let descriptor = this.descriptor;
    let hint = [];
    for key in ({"label", "icon", "title"})
      maphaskey(action, key) && (hint[tosym(key)] = action[key]);
    endfor
    descriptor['action] = hint;
    this.descriptor = descriptor;
    return this;
  endmethod
endobject
