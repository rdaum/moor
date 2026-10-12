object FORMAT_CODE [
  import_export_id -> "format_code",
  import_export_hierarchy -> {"format"}
]
  name: "Code Block Flyweight Delegate"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Constructors for opaque inline and block source code.";

  method mk owner: HACKER
    "Construct opaque source, with optional language and first source line number.";
    const {content, ?language = "", ?first_line = false} = args;
    typeof(content) in {TYPE_STR, TYPE_LIST} || raise(E_TYPE);
    typeof(language) in {TYPE_STR, TYPE_SYM} || raise(E_TYPE);
    const text = typeof(content) == TYPE_LIST ? $list_proto:join(content, "\n") | content;
    let fields = ['text -> text, 'language -> tostr(language)];
    if (!(typeof(first_line) == TYPE_BOOL && !first_line))
      fields['first_line] = first_line;
    endif
    return $document_node:mk('code_block, {}, fields);
  endmethod

  method inline owner: HACKER
    "Construct opaque inline source without parsing markup or evaluating substitutions.";
    const {text} = args;
    typeof(text) == TYPE_STR || raise(E_TYPE);
    return $document_node:mk('inline_code, {}, ['text -> text]);
  endmethod
endobject
