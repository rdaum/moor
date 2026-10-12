object DOCUMENT [
  import_export_id -> "document",
  import_export_hierarchy -> {"document"}
]
  name: "Document"
  parent: ROOT
  owner: HACKER
  readable: true

  method mk owner: HACKER
    "Construct a version 1 document from canonical block nodes. Validation is explicit.";
    const {blocks} = args;
    typeof(blocks) == TYPE_LIST || raise(E_TYPE);
    return <$document, .version = 1, blocks>;
  endmethod

  method validate owner: HACKER
    "Validate a complete document. Errors include a node path. No content methods run.";
    const {document} = args;
    $document_schema:encode(document);
    return true;
  endmethod

  method encode owner: HACKER
    "Return validated versioned metadata containing only inert JSON values.";
    const {document} = args;
    return $document_schema:encode(document);
  endmethod

  method decode owner: HACKER
    "Validate versioned metadata and reconstruct canonical flyweights without callbacks.";
    const {encoded} = args;
    return $document_schema:decode(encoded);
  endmethod

  method prepare owner: HACKER
    "Finalize references once, returning document, encoded metadata, and diagnostics.";
    "Invalid or excess references retain their labels. Other invalid structure raises an error.";
    const {document} = args;
    return $document_schema:prepare(document);
  endmethod

  method render owner: HACKER
    "Render a validated document as HTML, Djot, or plain text with its annotation table.";
    "Options are width (default 80) and code_line_numbers (default true).";
    "Rendering does not look up entities or invoke content callbacks.";
    const {document, content_type, ?options = []} = args;
    return $document_renderer:render(document, content_type, options);
  endmethod
  method render_many owner: HACKER
    "Finalize and encode once, rendering each distinct format and option set once.";
    "Requests are records with format and optional options. Results retain request order.";
    return $document_renderer:render_many(@args);
  endmethod
endobject
