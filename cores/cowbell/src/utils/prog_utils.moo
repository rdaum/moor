object PROG_UTILS [
  import_export_id -> "prog_utils",
  import_export_hierarchy -> {"utils"}
]
  name: "Programmer Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "Core programmer utilities for verb and object manipulation. Provides common functionality used by both programmer features and development tools - verb/property management, code search, object inspection, etc.";

  method grep_verb_code owner: ARCH_WIZARD
    "Search within a single verb's code for a pattern.";
    "Returns {line_number, truncated_line} if found, or 0 if no match.";
    "Args: {pattern, object, verb_num, casematters[, preserve_task_perms]}";
    {pattern, object, vnum, casematters, ?preserve_task_perms = false} = args;
    if (preserve_task_perms)
      stack = callers();
      caller == this || (length(stack) && stack[1][4] == this) || raise(E_PERM);
    else
      set_task_perms(caller_perms());
    endif
    "A missing verb is not a match; other failures should surface to the caller.";
    vc = `verb_code(object, vnum) ! E_VERBNF => false';
    if (!vc)
      return 0;
    endif
    "Quick check: does pattern exist anywhere in the verb code?";
    "This optimization from LambdaCore is much faster than checking line by line";
    suspend_if_needed();
    if (!index(tostr(@vc), pattern, casematters))
      return 0;
    endif
    suspend_if_needed();
    "Pattern exists somewhere, find which line";
    line_count = 0;
    for line in (vc)
      line_count = line_count + 1;
      if (line_count % 10 == 0)
        suspend_if_needed();
      endif
      if (match_pos = index(line, pattern, casematters))
        "Found the pattern - truncate and center around it";
        max_len = 50;
        line_len = length(line);
        if (line_len <= max_len)
          return {line_count, line};
        endif
        "Calculate window centered on the match";
        pattern_len = length(pattern);
        "Try to center the pattern in the window";
        start_pos = max(1, match_pos - (max_len - pattern_len) / 2);
        end_pos = min(line_len, start_pos + max_len - 1);
        "Adjust start if end hit the limit";
        if (end_pos == line_len && end_pos - start_pos < max_len - 1)
          start_pos = max(1, end_pos - max_len + 1);
        endif
        truncated = line[start_pos..end_pos];
        "Add ellipsis indicators";
        if (start_pos > 1)
          truncated = "..." + truncated;
        endif
        if (end_pos < line_len)
          truncated = truncated + "...";
        endif
        return {line_count, truncated};
      endif
    endfor
    return 0;
  endmethod

  method is_valid_prep owner: ARCH_WIZARD
    "Check if a string is a valid preposition spec";
    "Returns true if prep is 'none', 'any', or a valid preposition form";
    "Args: {prep_string}";
    set_task_perms(caller_perms());
    {prep_str} = args;
    typeof(prep_str) == TYPE_STR || return false;
    "Check for special cases";
    if (prep_str == "none" || prep_str == "any")
      return true;
    endif
    "Check against actual prepositions from the runtime";
    preps = prepositions();
    for prep_entry in (preps)
      {prep_id, short_form, all_forms} = prep_entry;
      "Check short form match";
      if (prep_str == short_form)
        return true;
      endif
      "Check against all forms";
      for form in (all_forms)
        if (prep_str == form)
          return true;
        endif
      endfor
    endfor
    return false;
  endmethod

  method grep_object owner: ARCH_WIZARD
    "Search all verbs on an object for a pattern.";
    "Returns list of matches: {{obj, verb_name, line_num, matching_line}, ...}";
    "Args: {pattern, search_obj, casematters[, run_as[, grants]]}";
    {pattern, search_obj, casematters, ?run_as = caller_perms(), ?grants = {}} = args;
    if (run_as != caller_perms() || grants)
      caller == $data_visor || raise(E_PERM);
    endif
    if (grants)
      set_task_perms(run_as, grants);
    else
      set_task_perms(run_as);
    endif
    if (!valid(search_obj))
      return {};
    endif
    matches = {};
    verb_count = 0;
    "Get metadata for all verbs on this object";
    verbs_metadata = this:get_verbs_metadata(search_obj, true);
    "Search each verb using its metadata";
    for metadata in (verbs_metadata)
      verb_count = verb_count + 1;
      if (verb_count % 5 == 0)
        suspend_if_needed();
      endif
      verb_name = metadata:name();
      verb_index = metadata:index();
      match_result = this:grep_verb_code(pattern, search_obj, verb_index, casematters, true);
      if (typeof(match_result) == TYPE_LIST)
        "Found a match - return just the essential data";
        {line_num, matching_line} = match_result;
        matches = {@matches, {search_obj, verb_name, line_num, matching_line}};
      endif
    endfor
    return matches;
  endmethod

  method format_line_numbers owner: ARCH_WIZARD
    "Format code lines with aligned line numbers.";
    "Returns list of strings with padded line numbers prepended.";
    "Args: {code_lines}";
    set_task_perms(caller_perms());
    {code_lines} = args;
    if (typeof(code_lines) != TYPE_LIST || length(code_lines) == 0)
      return {};
    endif
    num_lines = length(code_lines);
    num_width = length(tostr(num_lines));
    numbered_lines = {};
    for i in [1..num_lines]
      line_num_str = tostr(i);
      padding = $str_proto:space(num_width - length(line_num_str), " ");
      numbered_lines = {@numbered_lines, padding + line_num_str + ":  " + code_lines[i]};
    endfor
    return numbered_lines;
  endmethod

  method get_verb_metadata owner: ARCH_WIZARD
    "Get metadata for a verb as a flyweight with $verb delegate.";
    "Slots: owner_obj, location, name, verb_owner, flags, dobj, prep, iobj, index";
    "Args: {object, verb_name[, preserve_task_perms]}";
    {verb_obj, verb_name, ?preserve_task_perms = false} = args;
    if (preserve_task_perms)
      stack = callers();
      caller == this || (length(stack) && stack[1][4] == this) || raise(E_PERM);
    else
      set_task_perms(caller_perms());
    endif
    verb_info_data = verb_info(verb_obj, verb_name);
    {verb_owner, verb_flags, verb_names} = verb_info_data;
    verb_args_data = verb_args(verb_obj, verb_name);
    {dobj, prep, iobj} = verb_args_data;
    "Find the index of this verb in the object's verb list";
    try
      verb_list = verbs(verb_obj);
      verb_index = verb_name in verb_list;
      if (!verb_index && typeof(verb_name) == TYPE_STR)
        verb_index = verb_name:to_symbol() in verb_list;
      endif
    except e (E_PERM)
      "Can't list verbs due to permissions - use 0 as unknown index";
      verb_index = 0;
    endtry
    "Return as flyweight with $verb delegate and slots for metadata";
    return <$verb, .owner_obj = verb_obj, .location = verb_obj, .name = verb_name, .verb_owner = verb_owner, .flags = verb_flags, .dobj = dobj, .prep = prep, .iobj = iobj, .index = verb_index>;
  endmethod

  method get_verbs_metadata owner: ARCH_WIZARD
    "Get metadata for all verbs on an object as a list of flyweights.";
    "Returns list of $verb flyweights, one for each verb";
    "Args: {object[, preserve_task_perms]}";
    {verb_obj, ?preserve_task_perms = false} = args;
    if (preserve_task_perms)
      stack = callers();
      caller == this || (length(stack) && stack[1][4] == this) || raise(E_PERM);
    else
      set_task_perms(caller_perms());
    endif
    verb_list = verbs(verb_obj);
    metadata_list = {};
    for verb_index in [1..length(verb_list)]
      verb_name = verb_list[verb_index];
      verb_info_data = verb_info(verb_obj, verb_name);
      {verb_owner, verb_flags, verb_names} = verb_info_data;
      verb_args_data = verb_args(verb_obj, verb_name);
      {dobj, prep, iobj} = verb_args_data;
      metadata = <$verb, .owner_obj = verb_obj, .location = verb_obj, .name = verb_name, .verb_owner = verb_owner, .flags = verb_flags, .dobj = dobj, .prep = prep, .iobj = iobj, .index = verb_index>;
      metadata_list = {@metadata_list, metadata};
    endfor
    return metadata_list;
  endmethod

  method get_property_metadata owner: ARCH_WIZARD
    "Get metadata for a property as a flyweight with $property delegate.";
    "Slots: owner_obj, location, name, owner, perms, is_clear";
    "Args: {object, property_name}";
    set_task_perms(caller_perms());
    {prop_obj, prop_name} = args;
    prop_info_data = property_info(prop_obj, prop_name);
    {prop_owner, prop_perms} = prop_info_data;
    is_clear = is_clear_property(prop_obj, prop_name);
    "Return as flyweight with $property delegate and slots for metadata";
    return <$property, .owner_obj = prop_obj, .location = prop_obj, .name = prop_name, .owner = prop_owner, .perms = prop_perms, .is_clear = is_clear>;
  endmethod

  method test_format_line_numbers owner: HACKER
    "Test line number formatting";
    lines = {"line one", "line two", "line three"};
    numbered = this:format_line_numbers(lines);
    "Should return a list";
    if (typeof(numbered) != TYPE_LIST)
      return E_ASSERT;
    endif
    "Should have same length";
    if (length(numbered) != length(lines))
      return E_ASSERT;
    endif
    "First line should start with '1: '";
    first = numbered[1];
    if (typeof(first) != TYPE_STR || !index(first, "1:"))
      return E_ASSERT;
    endif
    "Last line should contain '3: '";
    last = numbered[3];
    if (typeof(last) != TYPE_STR || !index(last, "3:"))
      return E_ASSERT;
    endif
    return true;
  endmethod

  method test_is_valid_prep owner: HACKER
    "Test preposition validation";
    "Should accept 'none'";
    if (!this:is_valid_prep("none"))
      return E_ASSERT;
    endif
    "Should accept 'any'";
    if (!this:is_valid_prep("any"))
      return E_ASSERT;
    endif
    "Should accept 'with'";
    if (!this:is_valid_prep("with"))
      return E_ASSERT;
    endif
    "Should reject invalid prep";
    if (this:is_valid_prep("notaprep"))
      return E_ASSERT;
    endif
    return true;
  endmethod

  method parse_target_spec owner: ARCH_WIZARD
    "Parse a target specification for @show command.";
    "New syntax:";
    "  obj        -> object summary";
    "  obj.       -> local properties only";
    "  obj..      -> all properties (including inherited)";
    "  obj.name   -> specific property";
    "  obj:       -> local verbs only";
    "  obj::      -> all verbs (including inherited)";
    "  obj:name   -> specific verb";
    "  obj.:      -> local props + local verbs";
    "  obj.::     -> local props + all verbs";
    "  obj..:     -> all props + local verbs";
    "  obj..::    -> all props + all verbs";
    "Returns a map with keys: type, object_str, selectors";
    "  type is 'object for plain object, 'compound for selectors";
    "  selectors is a list of maps with keys: kind ('property or 'verb), inherited (bool), item_name (str or \"\")";
    "Returns 0 if invalid";
    spec = args[1]:trim();
    !spec && return 0;
    "Find where the object reference ends and selectors begin";
    "Selectors start at first . or : that isn't part of $name or #num";
    obj_end = 0;
    for i in [1..length(spec)]
      c = spec[i];
      if (c == "." || c == ":")
        "Check if this could be start of selector";
        "If we're at start or previous char suggests end of object ref, this is a selector";
        if (i == 1)
          "Can't start with selector";
          return 0;
        endif
        "Check if it's a $system.prop style reference - those have . but followed by alphanumeric";
        "For simplicity: first . or : after the object name starts selectors";
        "Object names are: #num, $name, or alphanumeric words";
        obj_end = i - 1;
        break;
      endif
    endfor
    "If no separators found, it's just an object";
    if (obj_end == 0)
      return ['type -> 'object, 'object_str -> spec, 'selectors -> {}];
    endif
    object_str = spec[1..obj_end]:trim();
    selector_str = spec[obj_end + 1..length(spec)];
    "Parse selector string into individual selectors";
    "Valid patterns: . .. .name : :: :name and combinations";
    selectors = {};
    i = 1;
    while (i <= length(selector_str))
      c = selector_str[i];
      if (c == ".")
        "Property selector";
        inherited = false;
        item_name = "";
        i = i + 1;
        "Check for double dot (inherited)";
        if (i <= length(selector_str) && selector_str[i] == ".")
          inherited = true;
          i = i + 1;
        endif
        "Check for property name";
        name_start = i;
        while (i <= length(selector_str) && selector_str[i] != "." && selector_str[i] != ":")
          i = i + 1;
        endwhile
        if (i > name_start)
          item_name = selector_str[name_start..i - 1];
        endif
        selectors = {@selectors, ['kind -> 'property, 'inherited -> inherited, 'item_name -> item_name]};
      elseif (c == ":")
        "Verb selector";
        inherited = false;
        item_name = "";
        i = i + 1;
        "Check for double colon (inherited)";
        if (i <= length(selector_str) && selector_str[i] == ":")
          inherited = true;
          i = i + 1;
        endif
        "Check for verb name";
        name_start = i;
        while (i <= length(selector_str) && selector_str[i] != "." && selector_str[i] != ":")
          i = i + 1;
        endwhile
        if (i > name_start)
          item_name = selector_str[name_start..i - 1];
        endif
        selectors = {@selectors, ['kind -> 'verb, 'inherited -> inherited, 'item_name -> item_name]};
      else
        "Invalid character in selector";
        return 0;
      endif
    endwhile
    if (!selectors)
      return 0;
    endif
    return ['type -> 'compound, 'object_str -> object_str, 'selectors -> selectors];
  endmethod

  method test_parse_target_spec owner: HACKER
    "Test target spec parsing";
    "Test object reference";
    result = this:parse_target_spec("me");
    result['type] == 'object || raise(E_ASSERT, "me should parse as object");
    result['object_str] == "me" || raise(E_ASSERT, "object_str should be 'me'");
    "Test named property selector";
    result = this:parse_target_spec("me.description");
    result['type] == 'compound || raise(E_ASSERT, "me.description should parse as compound selector");
    result['object_str] == "me" || raise(E_ASSERT, "object_str should be 'me'");
    length(result['selectors]) == 1 || raise(E_ASSERT, "me.description should have one selector");
    result['selectors][1]['kind] == 'property || raise(E_ASSERT, "me.description should select a property");
    result['selectors][1]['inherited] == false || raise(E_ASSERT, "me.description should not be inherited");
    result['selectors][1]['item_name] == "description" || raise(E_ASSERT, "item_name should be 'description'");
    "Test named verb selector";
    result = this:parse_target_spec("me:test");
    result['type] == 'compound || raise(E_ASSERT, "me:test should parse as compound selector");
    result['object_str] == "me" || raise(E_ASSERT, "object_str should be 'me'");
    length(result['selectors]) == 1 || raise(E_ASSERT, "me:test should have one selector");
    result['selectors][1]['kind] == 'verb || raise(E_ASSERT, "me:test should select a verb");
    result['selectors][1]['inherited] == false || raise(E_ASSERT, "me:test should not be inherited");
    result['selectors][1]['item_name] == "test" || raise(E_ASSERT, "item_name should be 'test'");
    "Test inherited property selector";
    result = this:parse_target_spec("me..description");
    result['type] == 'compound || raise(E_ASSERT, "me..description should parse as compound selector");
    result['selectors][1]['kind] == 'property || raise(E_ASSERT, "me..description should select a property");
    result['selectors][1]['inherited] == true || raise(E_ASSERT, "me..description should include inherited properties");
    result['selectors][1]['item_name] == "description" || raise(E_ASSERT, "item_name should be 'description'");
    "Test inherited verb selector";
    result = this:parse_target_spec("me::test");
    result['type] == 'compound || raise(E_ASSERT, "me::test should parse as compound selector");
    result['selectors][1]['kind] == 'verb || raise(E_ASSERT, "me::test should select a verb");
    result['selectors][1]['inherited] == true || raise(E_ASSERT, "me::test should include inherited verbs");
    result['selectors][1]['item_name] == "test" || raise(E_ASSERT, "item_name should be 'test'");
    "Test compound property and verb selectors";
    result = this:parse_target_spec("me.desc:verb");
    result['type] == 'compound || raise(E_ASSERT, "multiple selectors should parse as compound selector");
    length(result['selectors]) == 2 || raise(E_ASSERT, "multiple selectors should return two selectors");
    result['selectors][1]['kind] == 'property || raise(E_ASSERT, "first selector should be property");
    result['selectors][1]['item_name] == "desc" || raise(E_ASSERT, "first selector should be desc");
    result['selectors][2]['kind] == 'verb || raise(E_ASSERT, "second selector should be verb");
    result['selectors][2]['item_name] == "verb" || raise(E_ASSERT, "second selector should be verb");
    "Test local property-list selector";
    result = this:parse_target_spec("me.");
    result['type] == 'compound || raise(E_ASSERT, "me. should parse as compound selector");
    result['selectors][1]['kind] == 'property || raise(E_ASSERT, "me. should select properties");
    result['selectors][1]['inherited] == false || raise(E_ASSERT, "me. should select local properties");
    result['selectors][1]['item_name] == "" || raise(E_ASSERT, "me. should have empty item name");
    "Test inherited property-list selector";
    result = this:parse_target_spec("me..");
    result['type] == 'compound || raise(E_ASSERT, "me.. should parse as compound selector");
    result['selectors][1]['kind] == 'property || raise(E_ASSERT, "me.. should select properties");
    result['selectors][1]['inherited] == true || raise(E_ASSERT, "me.. should select inherited properties");
    result['selectors][1]['item_name] == "" || raise(E_ASSERT, "me.. should have empty item name");
    "Test local verb-list selector";
    result = this:parse_target_spec("me:");
    result['type] == 'compound || raise(E_ASSERT, "me: should parse as compound selector");
    result['selectors][1]['kind] == 'verb || raise(E_ASSERT, "me: should select verbs");
    result['selectors][1]['inherited] == false || raise(E_ASSERT, "me: should select local verbs");
    result['selectors][1]['item_name] == "" || raise(E_ASSERT, "me: should have empty item name");
    "Test inherited verb-list selector";
    result = this:parse_target_spec("me::");
    result['type] == 'compound || raise(E_ASSERT, "me:: should parse as compound selector");
    result['selectors][1]['kind] == 'verb || raise(E_ASSERT, "me:: should select verbs");
    result['selectors][1]['inherited] == true || raise(E_ASSERT, "me:: should select inherited verbs");
    result['selectors][1]['item_name] == "" || raise(E_ASSERT, "me:: should have empty item name");
    return true;
  endmethod

  method test_get_verb_metadata_accepts_string_name owner: ARCH_WIZARD
    "Regression test: string verb names still resolve against symbol-returning verbs().";
    target = #-1;
    try
      target = $thing:create();
      add_verb(target, {player, "rxd", "hit"}, {"this", "none", "none"});
      metadata = this:get_verb_metadata(target, "hit");
      $test_utils:assert_type(metadata, TYPE_FLYWEIGHT, "verb metadata should be a flyweight");
      $test_utils:assert_true(metadata:index() > 0, "string verb name should resolve to a direct local verb index");
      $test_utils:assert_eq(metadata:name(), "hit", "metadata should preserve requested verb name");
      $test_utils:assert_eq(metadata:verb_owner(), player, "metadata should read the direct verb owner");
    finally
      valid(target) && target:destroy();
    endtry
    return true;
  endmethod

  method eval_expression owner: ARCH_WIZARD
    "Evaluate one expression with the caller's permissions; return its value or raise its error.";
    "Args: {source[, selected_value]}. With a selected value, replace %# with its MOO literal.";
    "Requires programmer privileges. Literal input is parsed without executing code.";
    set_task_perms(caller_perms());
    const {source, ?selected_value = 0} = args;
    const programmer = caller_perms();
    (valid(programmer) && programmer.programmer) || raise(E_PERM,
      "Expression evaluation requires programmer privileges.");
    const code = length(args) > 1 ? strsub(source, "%#", toliteral(selected_value)) | source;
    try
      return fromliteral(code);
    except problem (E_INVARG, E_TYPE)
    endtry
    const answer = eval("return " + code + ";");
    answer[1] || raise(E_INVARG, "Invalid expression: " + answer[2]:join(" "));
    return answer[2];
  endmethod

  method test_eval_expression owner: HACKER
    "Check literals, selected values, caller permissions, and expression errors.";
    $test_utils:assert_eq(this:eval_expression("\"\""), "", "An empty string is a value");
    $test_utils:assert_eq(this:eval_expression("{1, 2}"), {1, 2}, "Parse literal data");
    $test_utils:assert_eq(this:eval_expression("3 * 4"), 12, "Evaluate computed values");
    $test_utils:assert_eq(this:eval_expression("%# + 1", 12), 13, "Use the selected value");
    $test_utils:assert_eq(this:eval_expression("length(%#)", "a\"b"), 3,
      "Quote selected strings as MOO literals");
    $test_utils:assert_eq(this:eval_expression("\"%#\""), "%#",
      "Without a selection, leave %# alone");
    $test_utils:assert_eq(this:eval_expression("task_perms()[1]"), task_perms()[1],
      "Evaluation must retain the caller's authority");
    $test_utils:assert_eq(`this:eval_expression("1 +") ! E_INVARG => E_INVARG', E_INVARG,
      "Report invalid expressions");
    $test_utils:assert_eq(`this:eval_expression("raise(E_RANGE)") ! E_RANGE => E_RANGE', E_RANGE,
      "Propagate execution errors");
    return true;
  endmethod

  method eval_literal owner: ARCH_WIZARD
    "Safely parse a MOO literal from a string, returning value and remaining text";
    "Returns {true, value, remaining_text} on success";
    "Returns {false, error_message, ''} on failure";
    "Accepts: numbers, strings, objects, lists, maps, symbols, errors";
    "If the whole string is not one literal, tries whitespace-delimited prefixes.";
    input_str = args[1]:trim();
    if (!input_str)
      return {false, "Empty input", ""};
    endif
    try
      return {true, fromliteral(input_str), ""};
    except e (E_INVARG, E_TYPE)
      parse_error = e[2];
    endtry
    words = input_str:words();
    word_count = length(words);
    while (word_count >= 1)
      literal_part = words[1..word_count]:join(" ");
      remaining = word_count < length(words) ? words[word_count + 1..$]:join(" ") | "";
      try
        return {true, fromliteral(literal_part), remaining};
      except e (E_INVARG, E_TYPE)
      endtry
      word_count = word_count - 1;
    endwhile
    return {false, parse_error, ""};
  endmethod

  method test_eval_literal owner: HACKER
    "Test literal evaluation";
    "Test simple integer literal";
    result = this:eval_literal("42");
    result[1] && result[2] == 42 || raise(E_ASSERT, "42 should parse as integer 42");
    "Test integer with remaining text";
    result = this:eval_literal("42 rc");
    result[1] && result[2] == 42 && result[3] == "rc" || raise(E_ASSERT, "42 rc should parse 42 with remainder rc");
    "Test list literal with embedded whitespace";
    result = this:eval_literal("{123, 2} rc");
    result[1] && result[2] == {123, 2} && result[3] == "rc" || raise(E_ASSERT, "{123, 2} rc should parse list literal with remainder rc");
    "Test symbol";
    result = this:eval_literal("'atom");
    result[1] && result[2] == 'atom || raise(E_ASSERT, "'atom should parse as symbol");
    "Test object literal";
    result = this:eval_literal(toliteral($root));
    result[1] && result[2] == $root || raise(E_ASSERT, "object literal should parse as object");
    return true;
  endmethod
endobject
