object FORMAT_ANNOTATION [
  import_export_id -> "format_annotation",
  import_export_hierarchy -> {"format"}
]
  name: "Semantic Annotation"
  parent: ROOT
  owner: HACKER
  readable: true

  method mk owner: HACKER
    "Construct a reference from an interaction-protocol descriptor and literal inline content.";
    const {label, descriptor} = args;
    typeof(descriptor) == TYPE_MAP || raise(E_TYPE);
    return $document_template:mk('reference, $format:inlines({label}),
      ['descriptor -> $document_schema:_descriptor_from_wire(descriptor)]);
  endmethod

  method object owner: ARCH_WIZARD
    "Capture an object reference with its authored label.";
    set_task_perms(caller_perms());
    const {target, ?label = target:name()} = args;
    is_anonymous(target) && return $format.paragraph:inline(label);
    return this:mk(label, ["kind" -> "object", "ref" -> $url_utils:to_curie_str(target), "objectKind" -> target:reference_kind()]);
  endmethod

  method command owner: HACKER
    "Capture the exact single-line command for explicit review.";
    const {command, ?label = command} = args;
    return this:mk(label, ["kind" -> "command", "command" -> command]);
  endmethod

  method command_template owner: HACKER
    "Describe authored argument fields and an exact parser template for command review.";
    const {template, fields, label} = args;
    typeof(template) == TYPE_STR && typeof(fields) == TYPE_MAP && length(fields) in {1, 2} || raise(E_INVARG);
    for slot in (mapkeys(fields))
      slot in {"dobj", "iobj"} || raise(E_INVARG);
      $match:input_context(template, slot);
    endfor
    return this:mk(label, ["kind" -> "command", "template" -> template, "arguments" -> fields]);
  endmethod

  method exit owner: ARCH_WIZARD
    "Use the registered exit descriptor and command supplied by its room.";
    set_task_perms(caller_perms());
    const {room, direction, ?label = direction} = args;
    const descriptor = room:exit_annotation(direction);
    return length(descriptor) ? this:mk(label, descriptor) | label;
  endmethod

  method help owner: HACKER
    "Qualify a topic by its help provider.";
    const {provider, topic, ?label = topic} = args;
    return this:mk(label, ["kind" -> "help", "provider" -> $url_utils:to_curie_str(provider), "topic" -> topic]);
  endmethod

  method verb owner: HACKER
    "Identify a verb on its receiver, optionally qualified by the definer.";
    const {receiver, name, ?label = tostr(receiver) + ":" + name, ?definer = false} = args;
    is_anonymous(receiver) && return $format.paragraph:inline(label);
    let descriptor = ["kind" -> "verb", "receiver" -> $url_utils:to_curie_str(receiver), "name" -> name];
    typeof(definer) == TYPE_OBJ && valid(definer) && (descriptor["definer"] = $url_utils:to_curie_str(definer));
    return this:mk(label, descriptor);
  endmethod

  method property owner: HACKER
    "Identify a property location, independently of its current value.";
    const {object, name, ?label = tostr(object) + "." + name} = args;
    is_anonymous(object) && return $format.paragraph:inline(label);
    return this:mk(label, ["kind" -> "property", "object" -> $url_utils:to_curie_str(object), "name" -> name]);
  endmethod

endobject
