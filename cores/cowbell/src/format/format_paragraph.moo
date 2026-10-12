object FORMAT_PARAGRAPH [
  import_export_id -> "format_paragraph",
  import_export_hierarchy -> {"format"}
]
  name: "Paragraph Content Flyweight Delegate"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  method mk owner: ARCH_WIZARD
    "Construct one paragraph from literal strings, inline nodes, and substitutions.";
    const parts = length(args) == 1 && typeof(args[1]) == TYPE_LIST ? args[1] | args;
    return $document_template:mk('paragraph, $format:inlines(parts));
  endmethod

  method inline owner: ARCH_WIZARD
    "Group inline content without introducing a paragraph boundary.";
    return $document_template:mk('inline, $format:inlines(args));
  endmethod
endobject
