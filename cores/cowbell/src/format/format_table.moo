object FORMAT_TABLE [
  import_export_id -> "format_table",
  import_export_hierarchy -> {"format"}
]
  name: "Table Content Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Flyweight delegate for table content in events.";

  method mk owner: HACKER
    "Construct a rectangular table from inline headers and rows of block-capable cells.";
    "Optional column alignments are start, center, or end. The caption is inline content.";
    const {headers, rows, ?alignments = {}, ?caption = {}} = args;
    typeof(headers) == TYPE_LIST && typeof(rows) == TYPE_LIST
      && typeof(alignments) == TYPE_LIST || raise(E_TYPE);
    let width = length(headers);
    !width && length(rows) && (width = length(rows[1]));
    !width && (width = length(alignments));
    width > 0 || raise(E_INVARG, "A table needs columns");
    !length(alignments) || length(alignments) == width
      || raise(E_INVARG, "Table alignments must match the column count");
    let columns = {};
    for i in [1..width]
      const alignment = length(alignments) ? alignments[i] | 'start;
      alignment in {'start, 'center, 'end} || raise(E_INVARG, "Invalid column alignment");
      columns = {@columns, ['align -> alignment]};
    endfor
    let records = {};
    for row in (rows)
      typeof(row) == TYPE_LIST && length(row) == width
        || raise(E_INVARG, "Table rows must match the column count");
      records = {@records, ['cells -> {
        ['blocks -> $format:blocks({cell})] for cell in (row)
      }]};
    endfor
    let fields = ['columns -> columns, 'caption -> $format:inlines({caption})];
    length(headers) && (fields['header] = {$format:inlines({header}) for header in (headers)});
    return $document_template:mk('table, records, fields);
  endmethod
endobject
