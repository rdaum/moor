object FORMAT_BLOCK [
  import_export_id -> "format_block",
  import_export_hierarchy -> {"format"}
]
  name: "Multiline Block Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Constructors for groups of document blocks.";

  method mk owner: HACKER
    "Group blocks. Each string argument is literal prose with blank lines between paragraphs.";
    let blocks = {};
    for part in (args)
      if (typeof(part) == TYPE_STR)
        blocks = {@blocks, @$format:literal_blocks(part)};
      else
        blocks = {@blocks, @$format:blocks({part})};
      endif
    endfor
    return $document_template:mk('blocks, blocks);
  endmethod
endobject
