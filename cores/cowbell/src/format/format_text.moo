object FORMAT_TEXT [
  import_export_id -> "format_text",
  import_export_hierarchy -> {"format"}
]
  name: "Literal Text"
  parent: ROOT
  owner: HACKER
  readable: true

  method mk owner: HACKER
    "Construct literal inline content, including explicit hard breaks for newlines.";
    const {text} = args;
    return $document_template:mk('inline, $format:literal_inline(text));
  endmethod
endobject
