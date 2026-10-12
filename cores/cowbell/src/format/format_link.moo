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

  method ambient_passage owner: HACKER
    "Create an ambient passage description with a exit command annotation.";
    "Args: {description, direction, room}. The link retains its originating room.";
    const {description, direction, room} = args;
    typeof(description) == TYPE_STR || raise(E_TYPE, "Description must be a string");
    typeof(direction) == TYPE_STR || raise(E_TYPE, "Direction must be a string");
    "Find the direction in the description";
    idx = index(description, direction);
    if (idx == 0)
      "Direction not found in description - append link at end";
      parts = {description, " (", $format.annotation:exit(room, direction), ")"};
    else
      "Split description around direction and insert link";
      before = description[1..idx - 1];
      "Get the actual text that matched (preserve original case)";
      matched = description[idx..idx + length(direction) - 1];
      after = description[idx + length(direction)..length(description)];
      parts = {before, $format.annotation:exit(room, direction, matched), after};
    endif
    return this:inline(parts);
  endmethod

  verb linkify_direction (none none none) owner: ARCH_WIZARD flags: "rxd"
    "Replace a direction word in a description with a exit command annotation.";
    "Args: (description, direction, room, ?lowercase=false) - returns inline flyweight or original string.";
    const {description, direction, room, ?lowercase = false} = args;
    typeof(description) == TYPE_STR || return description;
    typeof(direction) == TYPE_STR || return description;
    "Find the direction word in the description (case-insensitive)";
    pos = index(description, direction);
    !pos && return lowercase ? description:initial_lowercase() | description;
    "Split and create inline with link";
    before = pos > 1 ? description[1..pos - 1] | "";
    if (lowercase && length(before) > 0)
      before = before:initial_lowercase();
    endif
    after = pos + length(direction) <= length(description) ? description[pos + length(direction)..length(description)] | "";
    "Cowbell supplies both the invocation and the passage descriptor.";
    link = $format.annotation:exit(room, direction);
    return this:inline({before, link, after});
  endverb
endobject
