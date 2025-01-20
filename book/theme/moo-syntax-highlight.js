hljs.registerLanguage("moo", (hljs) => ({
  name: "MOO",
  case_insensitive: true,
  keywords: {
    keyword: "if while for return endif endwhile endfor else elseif in this try except finally endtry ANY",
    built_in: "abs acos add_property add_verb asin atan b binary_hash boot_player buffered_output_length c call_function caller_perms callers ceil children chparent clear_property connected_players connected_seconds connection_name connection_option connection_options cos cosh create crypt ctime d db_disk_size decode_binary delete_property delete_verb disassemble dump_database e encode_binary equal eval exp f floatstr floor flush_input force_input function_info i idle_seconds index is_clear_property is_member is_player k kill_task l length listappend listdelete listen listeners listinsert listset log log10 m match max max_object memory_usage min move n notify o object_bytes open_network_connection output_delimiters p parent pass players properties property_info q queue_info queued_tasks r raise random read recycle renumber reset_max_object resume rindex rmatch s seconds_left server_log server_version set_connection_option set_player_flag set_property_info set_task_perms set_verb_args set_verb_code set_verb_info setadd setremove shutdown sin sinh sqrt strcmp string_hash strsub substitute suspend t tan tanh task_id task_stack ticks_left time tofloat toint toliteral tonum toobj tostr trunc typeof u unlisten v valid value_bytes value_hash verb_args verb_code verb_info verbs false true player this caller verb args argstr dobj dobjstr prepstr iobj iobjstr $nothing $ambiguous_match $failed_match $system",
    type: "INT NUM FLOAT LIST MAP STR ANON OBJ ERR",
    variable: "E_NONE E_TYPE E_DIV E_PERM E_PROPNF E_VERBNF E_VARNF E_INVIND E_RECMOVE E_MAXREC E_RANGE E_ARGS E_NACC E_INVARG E_QUOTA E_FLOAT"
  },
  contains: [
    hljs.QUOTE_STRING_MODE,
    // numbers
    {
      // mdbook currently ships with highlightjs 10.1.1
      // highlightjs 11 replaced 'className' with 'scope'; specify both for forward compatibility
      className: "number",
      scope: "number",
      begin: '\\b[0-9]+\\.?[0-9]*([eE][+-]?[0-9]+)?\\b',
    },
    // numeric object references
    {
      className: "object",
      scope: "object",
      begin: "#-?[0-9]+\\b",
    },
    // corified references
    {
      className: "object",
      scope: "object",
      begin: "\\$\\w+\\b",
    },
    hljs.C_LINE_COMMENT_MODE,
  ],
}));

hljs.configure({languages: ["moo"]});
hljs.initHighlightingOnLoad();