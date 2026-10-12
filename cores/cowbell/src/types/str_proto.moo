object STR_PROTO [
  import_export_id -> "str_proto",
  import_export_hierarchy -> {"types"}
]
  name: "String Utilities"
  parent: ROOT
  location: PROTOTYPE_BOX
  owner: HACKER
  readable: true

  property _character_set (owner: HACKER, flags: "rc") = "\t !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
  property _character_set_in_ascii (owner: HACKER, flags: "rc") = {
    8,
    32,
    33,
    34,
    35,
    36,
    37,
    38,
    39,
    40,
    41,
    42,
    43,
    44,
    45,
    46,
    47,
    48,
    49,
    50,
    51,
    52,
    53,
    54,
    55,
    56,
    57,
    58,
    59,
    60,
    61,
    62,
    63,
    64,
    65,
    66,
    67,
    68,
    69,
    70,
    71,
    72,
    73,
    74,
    75,
    76,
    77,
    78,
    79,
    80,
    81,
    82,
    83,
    84,
    85,
    86,
    87,
    88,
    89,
    90,
    91,
    92,
    93,
    94,
    95,
    96,
    97,
    98,
    99,
    100,
    101,
    102,
    103,
    104,
    105,
    106,
    107,
    108,
    109,
    110,
    111,
    112,
    113,
    114,
    115,
    116,
    117,
    118,
    119,
    120,
    121,
    122,
    123,
    124,
    125,
    126
  };
  property _character_set_in_hex_ascii (owner: HACKER, flags: "rc") = {
    "08",
    "20",
    "21",
    "22",
    "23",
    "24",
    "25",
    "26",
    "27",
    "28",
    "29",
    "2A",
    "2B",
    "2C",
    "2D",
    "2E",
    "2F",
    "30",
    "31",
    "32",
    "33",
    "34",
    "35",
    "36",
    "37",
    "38",
    "39",
    "3A",
    "3B",
    "3C",
    "3D",
    "3E",
    "3F",
    "40",
    "41",
    "42",
    "43",
    "44",
    "45",
    "46",
    "47",
    "48",
    "49",
    "4A",
    "4B",
    "4C",
    "4D",
    "4E",
    "4F",
    "50",
    "51",
    "52",
    "53",
    "54",
    "55",
    "56",
    "57",
    "58",
    "59",
    "5A",
    "5B",
    "5C",
    "5D",
    "5E",
    "5F",
    "60",
    "61",
    "62",
    "63",
    "64",
    "65",
    "66",
    "67",
    "68",
    "69",
    "6A",
    "6B",
    "6C",
    "6D",
    "6E",
    "6F",
    "70",
    "71",
    "72",
    "73",
    "74",
    "75",
    "76",
    "77",
    "78",
    "79",
    "7A",
    "7B",
    "7C",
    "7D",
    "7E"
  };
  property alphabet (owner: HACKER, flags: "rc") = "abcdefghijklmnopqrstuvwxyz";
  property ascii (owner: HACKER, flags: "rc") = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";
  property digits (owner: HACKER, flags: "rc") = "0123456789";
  property tab (owner: HACKER, flags: "rc") = "\t";

  override description (owner: HACKER, flags: "rc") = "Prototype object for string utility methods and text manipulation operations.";

  method "capitalize capitalise" owner: HACKER
    "Capitalizes its argument.";
    string = args[1];
    if (string)
      let i = index("abcdefghijklmnopqrstuvwxyz", string[1], 1);
      if (i)
        string[1] = "ABCDEFGHIJKLMNOPQRSTUVWXYZ"[i];
      endif
    endif
    return string;
  endmethod

  method initial_lowercase owner: HACKER
    "Lowercases the first character of its argument.";
    string = args[1];
    if (string)
      let i = index("ABCDEFGHIJKLMNOPQRSTUVWXYZ", string[1], 1);
      if (i)
        string[1] = "abcdefghijklmnopqrstuvwxyz"[i];
      endif
    endif
    return string;
  endmethod

  method "centre center" owner: HACKER
    "Return text centered in a field of length len, using optional left and right fill strings.";
    "If len is negative and text is too long, truncate text to abs(len).";
    {text, len, ?lfill = " ", ?rfill = lfill} = args;
    out = tostr(text);
    abslen = abs(len);
    if (length(out) < abslen)
      return this:space((abslen - length(out)) / 2, lfill) + out + this:space((abslen - length(out) + 1) / -2, rfill);
    else
      return len > 0 ? out | out[1..abslen];
    endif
  endmethod

  method is_numeric owner: HACKER
    "Return true for an integer string with an optional sign and surrounding spaces.";
    const {string} = args;
    return !!match(string, "^ *[-+]?[0-9]+ *$");
  endmethod

  method literal_object owner: HACKER
    "Resolve a literal object string into an object reference.";
    "Supports #object ids, @player names, and $sysobj property paths; returns $failed_match when resolution fails.";
    string = args[1];
    if (!string)
      return $nothing;
    elseif (string[1] == "#")
      let object = this:toobj(string);
      if (E_TYPE != object)
        return object;
      endif
    elseif (string[1] == "@")
      return this:match_player(string[2..$], #0);
    elseif (string[1] == "$")
      string = string[2..$];
      let object = #0;
      while properties(1)
        let dot = index(string, ".");
        let pn = dot ? string[1..dot - 1] | string;
        try
          object = object.(pn);
        except (ANY)
          return $failed_match;
        endtry
        if (dot)
          string = string[dot + 1..$];
        else
          break properties;
        endif
      endwhile
      if (typeof(object) == TYPE_OBJ)
        return object;
      else
        return $failed_match;
      endif
    else
      return $failed_match;
    endif
  endmethod

  method render_as owner: HACKER
    "Render the given string part down into a proper string for the given content-type. For now this just returns it back, unmodified. Future versions could do escaping etc for HTML";
    return args[1];
  endmethod

  method space owner: HACKER
    "Return abs(width) characters of repeated fill, up to 100,000 characters.";
    "Negative widths anchor fill on the right. A string width uses its length.";
    "Empty fill raises E_INVARG when padding is required.";
    const {width, ?fill = " "} = args;
    const signed_width = typeof(width) == TYPE_STR ? length(width) | width;
    typeof(signed_width) == TYPE_INT || raise(E_TYPE, "Width must be an integer or string");
    typeof(fill) == TYPE_STR || raise(E_TYPE, "Fill must be a string");
    const size = abs(signed_width);
    size <= 100000 || raise(E_INVARG, "Width exceeds 100,000 characters");
    size == 0 && return "";
    fill || raise(E_INVARG, "Fill must not be empty");
    let padding = fill;
    while (length(padding) < size)
      padding = padding + padding;
    endwhile
    return signed_width < 0 ? padding[length(padding) - size + 1..$] | padding[1..size];
  endmethod

  method to_list owner: HACKER
    "Split on separator, retaining empty fields and ignoring case by default.";
    "An empty separator returns {subject}. Pass true as case_matters for exact matching.";
    const {subject, ?separator = " ", ?case_matters = false} = args;
    return this:split(subject, separator, case_matters);
  endmethod

  method toobj owner: HACKER
    ":toobj(objectid as string) => objectid";
    s = args[1];
    return match(s, "^ *#[-+]?[0-9]+ *$") ? toobj(s) | E_TYPE;
  endmethod

  method match_objid owner: HACKER
    "Find the first object identifier (#number or #uuid) within the given string.";
    "Returns a map with start/end offsets (1-based, inclusive), the matched text, and the identifier type.";
    "If no object identifier is present, returns false.";
    {s, ?anchored = false} = args;
    typeof(s) == TYPE_STR || raise(E_TYPE, "match_objid expects a string argument");
    len = length(s);
    if (len == 0)
      return false;
    endif
    fn is_digit(digit_char)
      return typeof(digit_char) == TYPE_STR && length(digit_char) == 1 && index("0123456789", digit_char) != 0;
    endfn
    fn is_hex(hex_char)
      return typeof(hex_char) == TYPE_STR && length(hex_char) == 1 && index("0123456789ABCDEFabcdef", hex_char) != 0;
    endfn
    fn is_alnum(alnum_char)
      return typeof(alnum_char) == TYPE_STR && length(alnum_char) == 1 && index("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz", alnum_char) != 0;
    endfn
    for idx in [1..len]
      if (s[idx] != "#")
        continue;
      endif
      if (idx > 1 && is_alnum(s[idx - 1]))
        continue;
      endif
      let uuid_bounds = this:match_uuobjid_at(s, idx);
      let end_index = 0;
      let match_type = 'numbered;
      if (uuid_bounds)
        end_index = uuid_bounds[2];
        match_type = 'uuid;
      else
        let pos = idx + 1;
        if (pos <= len && index("+-", s[pos]))
          pos = pos + 1;
        endif
        let digits_start = pos;
        while (pos <= len && is_digit(s[pos]))
          pos = pos + 1;
        endwhile
        if (pos > digits_start)
          end_index = pos - 1;
          match_type = 'numbered;
        endif
      endif
      if (!end_index)
        continue;
      endif
      if (end_index < len && is_alnum(s[end_index + 1]))
        continue;
      endif
      if (anchored)
        let prefix = idx > 1 ? s[1..idx - 1] | "";
        let suffix = end_index < len ? s[end_index + 1..$] | "";
        if (prefix:trim() || suffix:trim())
          continue;
        endif
      endif
      return ['text -> s[idx..end_index], 'start -> idx, 'end -> end_index, 'type -> match_type];
    endfor
    return false;
  endmethod

  method match_uuobjid_at owner: HACKER
    "Check for a uuobjid starting at position start_index, returning {start,end} or false.";
    {s, start_index} = args;
    typeof(s) == TYPE_STR || raise(E_TYPE, "Source must be string");
    typeof(start_index) == TYPE_INT || raise(E_TYPE, "Start index must be integer");
    start_index >= 1 || raise(E_INVARG);
    len = length(s);
    end_index = start_index + 17;
    if (end_index > len)
      return false;
    endif
    for offset in [1..6]
      if (!index("0123456789ABCDEFabcdef", s[start_index + offset]))
        return false;
      endif
    endfor
    if (s[start_index + 7] != "-")
      return false;
    endif
    for offset in [8..17]
      if (!index("0123456789ABCDEFabcdef", s[start_index + offset]))
        return false;
      endif
    endfor
    if (end_index < len && index("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz", s[end_index + 1]))
      return false;
    endif
    if (start_index > 1 && index("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz", s[start_index - 1]))
      return false;
    endif
    return {start_index, end_index};
  endmethod

  method parse_name_aliases owner: HACKER
    "Parse a name/alias specification. Supports both 'name:alias,alias' and 'name,alias,alias' formats.";
    {spec} = args;
    typeof(spec) == TYPE_STR || raise(E_TYPE, "Specification must be a string");
    trimmed = spec:trim();
    if (!trimmed)
      return {"", {}};
    endif
    "Split on colon if present (LambdaCore-style name:alias,alias)";
    colon_parts = this:to_list(trimmed, ":");
    use_colon = length(colon_parts) >= 2;
    if (use_colon)
      primary_raw = colon_parts[1];
      alias_input = colon_parts[2];
    else
      primary_raw = "";
      alias_input = trimmed;
    endif
    "Tokenize comma-separated values, respecting quoted commas and backslash escapes";
    tokens = {};
    current = "";
    in_quotes = false;
    i = 1;
    while (i <= length(alias_input))
      ch = alias_input[i];
      if (ch == "\\" && i < length(alias_input))
        current = current + alias_input[i + 1];
        i = i + 2;
        continue;
      endif
      if (ch == "\"")
        in_quotes = !in_quotes;
        current = current + ch;
        i = i + 1;
        continue;
      endif
      if (!in_quotes && ch == ",")
        tokens = {@tokens, current};
        current = "";
        i = i + 1;
        continue;
      endif
      current = current + ch;
      i = i + 1;
    endwhile
    tokens = {@tokens, current};
    "For comma-only form, tokens[1] is primary";
    if (!use_colon)
      primary_raw = tokens[1];
      tokens = length(tokens) > 1 ? tokens[2..$] | {};
    endif
    "Unquote primary";
    primary = primary_raw:trim();
    if (length(primary) >= 2 && primary[1] == "\"" && primary[$] == "\"")
      inner = "";
      j = 2;
      limit = length(primary) - 1;
      while (j <= limit)
        ch = primary[j];
        if (ch == "\\" && j < limit)
          inner = inner + primary[j + 1];
          j = j + 2;
          continue;
        endif
        inner = inner + ch;
        j = j + 1;
      endwhile
      primary = inner;
    endif
    "Unquote, trim, and dedupe aliases";
    aliases = {};
    for alias_raw in (tokens)
      alias = alias_raw:trim();
      if (length(alias) >= 2 && alias[1] == "\"" && alias[$] == "\"")
        inner = "";
        j = 2;
        limit = length(alias) - 1;
        while (j <= limit)
          ch = alias[j];
          if (ch == "\\" && j < limit)
            inner = inner + alias[j + 1];
            j = j + 2;
            continue;
          endif
          inner = inner + ch;
          j = j + 1;
        endwhile
        alias = inner;
      endif
      if (!alias || alias == primary)
        continue;
      endif
      if (!(alias in aliases))
        aliases = {@aliases, alias};
      endif
    endfor
    "If primary is missing, promote first alias";
    if (!primary)
      primary = aliases ? aliases[1] | "";
      aliases = primary ? aliases[2..$] | {};
    endif
    return {primary, aliases};
  endmethod

  method trim owner: HACKER
    ":trim (string [, chars]) -- remove leading and trailing whitespace";
    "";
    "`chars' should be a string of characters to trim; defaults to space, tab, newline, carriage return.";
    "Returns a copy of string with all leading and trailing copies of those characters removed.";
    "For example, $string:trim(\"***foo***\", \"*\") => \"foo\".";
    {string, ?chars = " \t\n"} = args;
    !string && return "";
    len = length(string);
    "Find first non-whitespace character";
    start = 1;
    while (start <= len && index(chars, string[start]) > 0)
      start = start + 1;
    endwhile
    "Find last non-whitespace character";
    finish = len;
    while (finish >= start && index(chars, string[finish]) > 0)
      finish = finish - 1;
    endwhile
    start > finish && return "";
    return string[start..finish];
  endmethod

  method triml owner: HACKER
    ":triml(string [, space]) -- remove leading spaces";
    "";
    "`space' should be a character (single-character string); it defaults to \" \".  Returns a copy of string with all leading copies of that character removed.  For example, $string:triml(\"***foo***\", \"*\") => \"foo***\".";
    {string, ?space = " "} = args;
    return string[match(string, tostr("[^", space, "]%|$"))[1]..length(string)];
  endmethod

  method trimr owner: HACKER
    ":trimr(string [, space]) -- remove trailing spaces";
    "";
    "`space' should be a character (single-character string); it defaults to \" \".  Returns a copy of string with all trailing copies of that character removed.  For example, $string:trimr(\"***foo***\", \"*\") => \"***foo\".";
    {string, ?space = " "} = args;
    return string[1..rmatch(string, tostr("[^", space, "]%|^"))[2]];
  endmethod

  method "uppercase lowercase" owner: HACKER
    "lowercase(string) -- returns a lowercase version of the string.";
    "uppercase(string) -- returns the uppercase version of the string.";
    string = args[1];
    caps = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    lower = "abcdefghijklmnopqrstuvwxyz";
    from = caps;
    to = lower;
    if (verb == "uppercase")
      from = lower;
      to = caps;
    endif
    return strtr(string, from, to, 1);
  endmethod

  method word_start owner: HACKER
    "This breaks up the argument string into words, returning a list of indices into argstr corresponding to the starting points of each of the arguments.";
    rest = args[1];
    wstart = match(rest, "[^ ]%|$")[1];
    wbefore = wstart - 1;
    rest[1..wbefore] = "";
    if (!rest)
      return {};
    endif
    quote = 0;
    wslist = {};
    pattern = " +%|\\.?%|\"";
    m = 0;
    char = 0;
    while (m = match(rest, pattern))
      char = rest[m[1]];
      if (char == " ")
        wslist = {@wslist, {wstart, wbefore + m[1] - 1}};
        wstart = wbefore + m[2] + 1;
      elseif (char == "\"")
        pattern = (quote = !quote) ? "\\.?%|\"" | " +%|\\.?%|\"";
      endif
      rest[1..m[2]] = "";
      wbefore = wbefore + m[2];
    endwhile
    return rest || char != " " ? {@wslist, {wstart, wbefore + length(rest)}} | wslist;
  endmethod

  method words owner: HACKER
    "This breaks up the argument string into words, the resulting list being obtained exactly the way the command line parser obtains `args' from `argstr'.";
    rest = args[1];
    rest[1..match(rest, "^ *")[2]] = "";
    if (!rest)
      return {};
    endif
    quote = 0;
    toklist = {};
    token = "";
    pattern = " +%|\\.?%|\"";
    m = 0;
    char = 0;
    while (m = match(rest, pattern))
      char = rest[m[1]];
      token = token + rest[1..m[1] - 1];
      if (char == " ")
        toklist = {@toklist, token};
        token = "";
      elseif (char == "\"")
        pattern = (quote = !quote) ? "\\.?%|\"" | " +%|\\.?%|\"";
      elseif (m[1] < m[2])
        token = token + rest[m[2]];
      endif
      rest[1..m[2]] = "";
    endwhile
    return rest || char != " " ? {@toklist, token + rest} | toklist;
  endmethod

  method append_to_paragraph owner: HACKER
    "Given arguments which are list of strings, appends.";
    "e.g.  \"dog\":append_to_paragraph() => {\"dog\"}";
    "      \"dog\":append_to_paragraph(\"cats and\") => {\"cats and dogs\"}";
    "      \"dog\":append_to_paragraph(\"cats get chased by:\", \"\") => { \"cats get chased by:\", \"dogs\"}";
    length(args) == 1 && return args;
    length(args) == 2 && return {args[2] + args[1]};
    head = args[2..length(args) - 1];
    tail = args[length(args)] + args[1];
    return {@head, tail};
  endmethod

  method parse_verbref owner: HACKER
    "Split object:verb syntax without resolving the object; matching reports missing references.";
    const {reference} = args;
    const colon = index(reference, ":");
    colon || return false;
    const object = reference[1..colon - 1]:trim();
    const verbname = reference[colon + 1..$]:trim();
    object && verbname || return false;
    return {object, verbname};
  endmethod

  method split owner: HACKER
    "Split on delimiter, retaining empty fields. Matching is case-sensitive by default.";
    "An empty delimiter returns {string}. Pass false as case_matters to ignore case.";
    const {string, delimiter, ?case_matters = true} = args;
    (typeof(string) == TYPE_STR && typeof(delimiter) == TYPE_STR) || raise(E_TYPE);
    const delimiter_length = length(delimiter);
    delimiter_length == 0 && return {string};
    if (delimiter_length == 1 && case_matters)
      return explode(string, delimiter, 1);
    endif
    let parts = {};
    let remaining = string;
    while (true)
      const position = index(remaining, delimiter, case_matters);
      position || return {@parts, remaining};
      parts = {@parts, remaining[1..position - 1]};
      remaining = remaining[position + delimiter_length..$];
    endwhile
  endmethod

  method join_list owner: HACKER
    "Compatibility entry point for $str_proto:join_list(list, separator).";
    "Prefer list:join(separator) for primitive dispatch.";
    const {items, separator} = args;
    return items:join(separator);
  endmethod

  method starts_with owner: HACKER
    "starts_with(string, prefix) => true if string starts with prefix";
    "Example: \"hello world\":starts_with(\"hello\") => true";
    return length(args[2]) <= length(args[1]) && args[1][1..length(args[2])] == args[2];
  endmethod

  method ends_with owner: HACKER
    "ends_with(string, suffix) => true if string ends with suffix";
    "Example: \"hello world\":ends_with(\"world\") => true";
    return length(args[2]) <= length(args[1]) && args[1][length(args[1]) - length(args[2]) + 1..$] == args[2];
  endmethod

  method contains owner: HACKER
    "contains(string, substring) => true if string contains substring";
    "Example: \"hello world\":contains(\"lo wo\") => true";
    return index(args[1], args[2]) != 0;
  endmethod

  method replace_all owner: HACKER
    "Replace all occurrences of old with new. Matching is case-sensitive by default.";
    "An empty old string leaves the input unchanged. Pass false to ignore case.";
    const {string, old, new, ?case_matters = true} = args;
    return strsub(string, old, new, case_matters);
  endmethod

  method reverse owner: HACKER
    "reverse(string) => string with characters in reverse order";
    "Example: \"hello\":reverse() => \"olleh\"";
    {instr} = args;
    out = "";
    for i in [1..length(instr)]
      out = instr[i] + out;
    endfor
    return out;
  endmethod

  method repeat owner: HACKER
    "repeat(string, count) => string repeated count times";
    "Example: \"ab\":repeat(3) => \"ababab\"";
    {string, count} = args;
    if (count <= 0)
      return "";
    endif
    result = "";
    for i in [1..count]
      result = result + string;
    endfor
    return result;
  endmethod

  method char_count owner: HACKER
    "char_count(string, character) => count of character occurrences in string";
    "Example: \"hello\":char_count(\"l\") => 2";
    {string, char} = args;
    count = 0;
    for i in [1..length(string)]
      if (string[i] == char)
        count = count + 1;
      endif
    endfor
    return count;
  endmethod

  method substring owner: HACKER
    "substring(string, start, length) => substring starting at start for length characters";
    "Example: \"hello world\":substring(7, 5) => \"world\"";
    {string, start, len} = args;
    return string[start..start + len - 1];
  endmethod

  method pad_left owner: HACKER
    "pad_left(string, width, fill) => string padded on left to width with fill character";
    "Example: \"hello\":pad_left(10, \"*\") => \"*****hello\"";
    {string, width, ?fill = " "} = args;
    padding_needed = width - length(string);
    if (padding_needed <= 0)
      return string;
    endif
    return this:space(padding_needed, fill) + string;
  endmethod

  method pad_right owner: HACKER
    "pad_right(string, width, fill) => string padded on right to width with fill character";
    "Example: \"hello\":pad_right(10, \"*\") => \"hello*****\"";
    {string, width, ?fill = " "} = args;
    padding_needed = width - length(string);
    if (padding_needed <= 0)
      return string;
    endif
    return string + this:space(padding_needed, fill);
  endmethod

  method words_list owner: ARCH_WIZARD
    "Split on spaces, tabs, carriage returns, and newlines. Ignore empty fields.";
    const {string} = args;
    return explode(strtr(string, "\t\r\n", "   ", 1), " ");
  endmethod

  method map_chars owner: ARCH_WIZARD
    "map_chars(string, function) => string with function applied to each character";
    "Example: \"hello\":map_chars({c} => uppercase(c)) => \"HELLO\"";
    set_task_perms(caller_perms());
    {string, func} = args;
    result = "";
    for i in [1..length(string)]
      result = result + func(string[i]);
    endfor
    return result;
  endmethod

  method filter_chars owner: ARCH_WIZARD
    "filter_chars(string, predicate) => string with only characters matching predicate";
    "Example: \"abc123def\":filter_chars({c} => c in \"abcdefghijklmnopqrstuvwxyz\") => \"abcdef\"";
    set_task_perms(caller_perms());
    {string, pred} = args;
    result = "";
    for i in [1..length(string)]
      if (pred(string[i]))
        result = result + string[i];
      endif
    endfor
    return result;
  endmethod

  method title_case owner: HACKER
    "title_case(string) => string with first letter of each word capitalized";
    "Example: \"hello world\":title_case() => \"Hello World\"";
    string = args[1];
    words = this:words_list(string);
    result = {};
    for word in (words)
      result = {@result, this:capitalize(word)};
    endfor
    return result:join(" ");
  endmethod

  method from_seconds owner: HACKER
    ":from_seconds(number of seconds) => returns a string containing the rough increment of days, or hours if less than a day, or minutes if less than an hour, or lastly in seconds.";
    ":from_seconds(86400) => \"a day\"";
    ":from_seconds(7200)  => \"two hours\"";
    minute = 60;
    hour = 60 * minute;
    day = 24 * hour;
    secs = args[1];
    if (secs >= day)
      count = secs / day;
      unit = "day";
      article = "a";
    elseif (secs >= hour)
      count = secs / hour;
      unit = "hour";
      article = "an";
    elseif (secs >= minute)
      count = secs / minute;
      unit = "minute";
      article = "a";
    else
      count = secs;
      unit = "second";
      article = "a";
    endif
    if (count == 1)
      time = tostr(article, " ", unit);
    else
      time = tostr(count, " ", unit, "s");
    endif
    return time;
  endmethod

  method parse_time_of_day owner: HACKER
    "Parse HH:MM:SS time string and return next occurrence as Unix timestamp.";
    "Example: \"14:30:00\":parse_time_of_day() returns timestamp for next 2:30 PM.";
    {time_str} = args;
    typeof(time_str) == TYPE_STR || raise(E_TYPE, "Time must be string");
    parts = time_str:split(":");
    length(parts) == 3 || raise(E_INVARG, "Time must be HH:MM:SS format");
    hours = toint(parts[1]);
    minutes = toint(parts[2]);
    seconds = toint(parts[3]);
    hours >= 0 && hours < 24 || raise(E_INVARG, "Hours must be 0-23");
    minutes >= 0 && minutes < 60 || raise(E_INVARG, "Minutes must be 0-59");
    seconds >= 0 && seconds < 60 || raise(E_INVARG, "Seconds must be 0-59");
    "Calculate seconds from midnight";
    target_seconds = hours * 3600 + minutes * 60 + seconds;
    "Get current time and calculate today's occurrence";
    now = time();
    ct = ctime(now);
    current_hour = toint(ct[12..13]);
    current_minute = toint(ct[15..16]);
    current_second = toint(ct[18..19]);
    current_seconds = current_hour * 3600 + current_minute * 60 + current_second;
    "If target time has passed today, schedule for tomorrow";
    if (target_seconds <= current_seconds)
      return now + (86400 - current_seconds + target_seconds);
    else
      return now + (target_seconds - current_seconds);
    endif
  endmethod

  method compose owner: HACKER
    "Return the given string unchanged for compatibility with composable content APIs.";
    return args[1];
  endmethod

  method indefinite_article owner: HACKER
    "Return the appropriate indefinite article ('a' or 'an') for this string based on first letter.";
    s = args[1];
    typeof(s) == TYPE_STR || raise(E_TYPE("Expected string"));
    !s && return "a";
    first_char = s[1..1]:lowercase();
    return first_char in {"a", "e", "i", "o", "u"} ? "an" | "a";
  endmethod

  method with_indefinite_article owner: HACKER
    "Return this string prefixed with the appropriate indefinite article.";
    s = args[1];
    typeof(s) == TYPE_STR || raise(E_TYPE("Expected string"));
    !s && return "a ";
    return s:indefinite_article() + " " + s;
  endmethod

  method parse_curie owner: HACKER
    "Parse a CURIE string into an object reference.";
    "Supports: oid:N, uuid:XXXXXX-XXXXXXXXXX, sysobj:name.path, match(\"string\")";
    "Returns the object or false if parsing fails.";
    "Usage: \"oid:42\":parse_curie() => #42";
    "For match() strings, optional second arg provides context: \"match(\\\"sword\\\")\":parse_curie(player) => #123";
    {curie_str, ?context = player} = args;
    typeof(curie_str) == TYPE_STR || raise(E_TYPE, "CURIE must be a string");
    "Check for oid: prefix";
    if (curie_str:starts_with("oid:"))
      oid_part = curie_str[5..$];
      if (oid_part:is_numeric())
        return toobj("#" + oid_part);
      endif
      return false;
    endif
    "Check for uuid: prefix";
    if (curie_str:starts_with("uuid:"))
      uuid_part = curie_str[6..$];
      "Try to construct uuid object reference";
      try
        return toobj("#" + uuid_part);
      except (ANY)
        return false;
      endtry
    endif
    "Check for sysobj: prefix";
    if (curie_str:starts_with("sysobj:"))
      sysobj_part = curie_str[8..$];
      "Navigate from #0 through property chain";
      try
        let target = #0;
        parts = sysobj_part:split(".");
        for part in (parts)
          target = target.(part);
        endfor
        return typeof(target) == TYPE_OBJ ? target | false;
      except (ANY)
        return false;
      endtry
    endif
    "Check for match(\"...\") format";
    if (curie_str:starts_with("match(\"") && curie_str:ends_with("\")"))
      match_str = curie_str[8..length(curie_str) - 2];
      "Use match_object with provided context";
      if (!valid(context))
        return false;
      endif
      try
        return $match:match_object(match_str, context);
      except (ANY)
        return false;
      endtry
    endif
    return false;
  endmethod

  method test_case_whitespace_and_padding owner: HACKER
    "Cover case, trimming, spacing, and padding helpers.";
    $test_utils:assert_true(equal("cowbell":capitalize(), "Cowbell"), "capitalize lowercase word");
    $test_utils:assert_true(equal("Cowbell":initial_lowercase(), "cowbell"), "initial_lowercase uppercase word");
    $test_utils:assert_true(equal("hELLo":lowercase(), "hello"), "lowercase mixed word");
    $test_utils:assert_true(equal("hELLo":uppercase(), "HELLO"), "uppercase mixed word");
    $test_utils:assert_true(equal("  cowbell  ":trim(), "cowbell"), "trim default whitespace");
    $test_utils:assert_true(equal("***cowbell***":trim("*"), "cowbell"), "trim custom character");
    $test_utils:assert_true(equal("***cowbell***":triml("*"), "cowbell***"), "triml custom character");
    $test_utils:assert_true(equal("***cowbell***":trimr("*"), "***cowbell"), "trimr custom character");
    $test_utils:assert_true(equal("moo":center(7), "  moo  "), "center default fill");
    $test_utils:assert_true(equal("moo":centre(-2), "mo"), "centre negative length truncates");
    $test_utils:assert_true(equal(this:space(5, "*"), "*****"), "space custom fill");
    $test_utils:assert_true(equal("hi":pad_left(5), "   hi"), "pad_left default fill");
    $test_utils:assert_true(equal("hi":pad_right(5, "."), "hi..."), "pad_right custom fill");
    $test_utils:assert_true(equal("hello world":title_case(), "Hello World"), "title_case basic words");
    return true;
  endmethod

  method test_list_word_and_paragraph_helpers owner: HACKER
    "Cover list conversion, word parsing, and paragraph append helpers.";
    $test_utils:assert_true(equal("a::b::":to_list("::"), {"a", "b", ""}), "to_list preserves trailing empty part");
    $test_utils:assert_true(equal(this:join_list({"a", "b", "c"}, "|"), "a|b|c"), "join_list joins with separator");
    $test_utils:assert_true(equal("  alpha beta  ":words(), {"alpha", "beta"}), "words trims and splits");
    $test_utils:assert_true(equal("alpha \"big dog\"":words(), {"alpha", "big dog"}), "words respects quotes");
    $test_utils:assert_true(equal("  alpha beta":word_start(), {{3, 7}, {9, 12}}), "word_start reports source offsets");
    $test_utils:assert_true(equal(" hello   world test ":words_list(), {"hello", "world", "test"}), "words_list drops empty fields");
    $test_utils:assert_true(equal("\tone\ttwo\r\nthree\n":words_list(), {"one", "two", "three"}),
      "words_list splits all documented whitespace");
    $test_utils:assert_true(equal(" \t\r\n":words_list(), {}), "Whitespace-only input");
    $test_utils:assert_true(equal("":append_to_paragraph(), {""}), "append_to_paragraph empty receiver");
    $test_utils:assert_true(equal("dog":append_to_paragraph("cats and "), {"cats and dog"}), "append_to_paragraph appends to last line");
    $test_utils:assert_true(equal("dog":append_to_paragraph("cats and, also...", "a ", ""), {"cats and, also...", "a ", "dog"}), "append_to_paragraph preserves intermediate lines");
    return true;
  endmethod

  method test_object_reference_helpers owner: HACKER
    "Cover object-reference parsing helpers.";
    $test_utils:assert_true(equal("#1":toobj(), #1), "toobj numbered object");
    $test_utils:assert_true(equal("not an object":toobj(), E_TYPE), "toobj rejects non-object strings");
    $test_utils:assert_true(equal("#1":literal_object(), #1), "literal_object numbered object");
    $test_utils:assert_true(equal("$root":literal_object(), $root), "literal_object sysobj property");
    $test_utils:assert_true(equal("$str_proto":literal_object(), $str_proto), "literal_object str_proto sysobj property");
    $test_utils:assert_true(equal("missing":literal_object(), $failed_match), "literal_object rejects bare strings");
    $test_utils:assert_true(equal("raw":render_as('text), "raw"), "render_as returns strings unchanged");
    $test_utils:assert_true(equal("raw":compose(), "raw"), "compose returns strings unchanged");
    return true;
  endmethod

  method test_parse_verbref owner: HACKER
    "Cover MOO verb-reference parsing.";
    $test_utils:assert_true(equal("#1":parse_verbref(), false), "missing colon should fail");
    $test_utils:assert_true(equal(":":parse_verbref(), false), "empty object and verb should fail");
    $test_utils:assert_true(equal("$str_proto:look_self":parse_verbref(), {"$str_proto", "look_self"}), "sysobj verbref should preserve the reference for matching");
    $test_utils:assert_true(equal("$missing_object:sit":parse_verbref(), {"$missing_object", "sit"}), "unknown object is valid reference syntax");
    $test_utils:assert_true(equal("$missing_object:":parse_verbref(), false), "missing verb remains invalid syntax");
    $test_utils:assert_true(equal("  $str_proto : look_self  ":parse_verbref(), {"$str_proto", "look_self"}), "trim reference components");
    $test_utils:assert_true(equal("#1:look_self":parse_verbref(), {"#1", "look_self"}), "numbered verbref should preserve object string");
    $test_utils:assert_true(equal("honk:look_self":parse_verbref(), {"honk", "look_self"}), "named verbref should preserve object string");
    return true;
  endmethod

  method test_split_search_and_transform_helpers owner: HACKER
    "Cover split/search/replace and string transforms.";
    $test_utils:assert_true(equal("a,b,c":split(","), {"a", "b", "c"}), "split basic delimiter");
    $test_utils:assert_true(equal("a,,c":split(","), {"a", "", "c"}), "split preserves empty parts");
    $test_utils:assert_true(equal("hello":split(","), {"hello"}), "split without delimiter match");
    $test_utils:assert_true(equal("a::b::c":split("::"), {"a", "b", "c"}), "split multi-character delimiter");
    $test_utils:assert_true("hello world":starts_with("hello"), "starts_with positive");
    $test_utils:assert_false("hello world":starts_with("world"), "starts_with negative");
    $test_utils:assert_true("hello world":ends_with("world"), "ends_with positive");
    $test_utils:assert_false("hello world":ends_with("hello"), "ends_with negative");
    $test_utils:assert_true("hello world":contains("lo wo"), "contains positive");
    $test_utils:assert_false("hello world":contains("xyz"), "contains negative");
    $test_utils:assert_true(equal("hello world":replace_all("l", "x"), "hexxo worxd"), "replace_all basic replacement");
    $test_utils:assert_true(equal("hello":replace_all("", "x"), "hello"), "replace_all empty old string");
    $test_utils:assert_true(equal("hello":reverse(), "olleh"), "reverse basic string");
    $test_utils:assert_true(equal("ab":repeat(3), "ababab"), "repeat positive count");
    $test_utils:assert_true(equal("hello":repeat(0), ""), "repeat zero count");
    $test_utils:assert_true(equal("hello":char_count("l"), 2), "char_count repeated character");
    $test_utils:assert_true(equal("hello world":substring(7, 5), "world"), "substring range");
    $test_utils:assert_true(equal("hello":map_chars({c0} => this:uppercase(c0)), "HELLO"), "map_chars uppercase function");
    $test_utils:assert_true(equal("abc123def":filter_chars({c1} => c1 in "abcdefghijklmnopqrstuvwxyz"), "abcdef"), "filter_chars letters");
    const expected = task_perms()[1];
    $test_utils:assert_false(expected.wizard, "Permission checks need a non-wizard caller");
    "a":map_chars({c2} => caller_perms() == expected ? c2 | "x") == "a" || raise(E_ASSERT("map_chars callback should run with caller perms"));
    "a":filter_chars({c3} => caller_perms() == expected) == "a" || raise(E_ASSERT("filter_chars callback should run with caller perms"));
    return true;
  endmethod

  method test_match_objid_helpers owner: HACKER
    "Cover numeric and UUID object-id matching helpers.";
    result = this:match_objid("#42");
    $test_utils:assert_true(result, "match_objid should find bare numbered object ids");
    $test_utils:assert_true(equal(result['type], 'numbered), "numbered object-id type");
    $test_utils:assert_true(equal(result['text], "#42"), "numbered object-id text");
    $test_utils:assert_true(equal(result['start], 1), "numbered object-id start");
    $test_utils:assert_true(equal(result['end], 3), "numbered object-id end");
    result = this:match_objid("Before #123 after");
    $test_utils:assert_true(result, "match_objid should find numbered object ids in text");
    $test_utils:assert_true(equal(result['start], 8), "embedded object-id start");
    $test_utils:assert_true(equal(result['end], 11), "embedded object-id end");
    result = this:match_objid("   #77   ", true);
    $test_utils:assert_true(result, "anchored match_objid should ignore surrounding whitespace");
    $test_utils:assert_true(equal(result['text], "#77"), "anchored object-id text");
    $test_utils:assert_false(this:match_objid("x#77"), "match_objid should reject alphanumeric left boundary");
    $test_utils:assert_false(this:match_objid("#77x"), "match_objid should reject alphanumeric right boundary");
    $test_utils:assert_false(this:match_objid("#notanumber"), "match_objid should reject non-numeric numbered ids");
    uuid_str = "#00007D-99E53ABE55";
    result = this:match_objid(uuid_str);
    $test_utils:assert_true(result, "match_objid should find UUID object ids");
    $test_utils:assert_true(equal(result['type], 'uuid), "UUID object-id type");
    $test_utils:assert_true(equal(result['text], uuid_str), "UUID object-id text");
    $test_utils:assert_true(equal(result['start], 1), "UUID object-id start");
    $test_utils:assert_true(equal(result['end], 18), "UUID object-id end");
    $test_utils:assert_true(equal(this:match_uuobjid_at(uuid_str, 1), {1, 18}), "match_uuobjid_at bare UUID");
    $test_utils:assert_false(this:match_uuobjid_at("x" + uuid_str, 2), "match_uuobjid_at should reject alphanumeric left boundary");
    return true;
  endmethod

  method test_parse_name_aliases owner: HACKER
    "Cover colon and comma name/alias specifications.";
    $test_utils:assert_true(equal(this:parse_name_aliases("lamp:light, lamp,shiny"), {"lamp", {"light", "shiny"}}), "colon aliases trim and dedupe");
    $test_utils:assert_true(equal(this:parse_name_aliases(":alpha,beta"), {"alpha", {"beta"}}), "missing colon primary promotes first alias");
    $test_utils:assert_true(equal(this:parse_name_aliases("Porcupine:\"Karl Porcupine\",\"You can pet this Porcupine, I bet!\""), {"Porcupine", {"Karl Porcupine", "You can pet this Porcupine, I bet!"}}), "quoted colon aliases");
    $test_utils:assert_true(equal(this:parse_name_aliases("\"Standalone Thing\""), {"Standalone Thing", {}}), "standalone quoted name");
    $test_utils:assert_true(equal(this:parse_name_aliases("test,bonk"), {"test", {"bonk"}}), "comma aliases");
    $test_utils:assert_true(equal(this:parse_name_aliases("\"Quoted Thing\",alias1,alias2"), {"Quoted Thing", {"alias1", "alias2"}}), "quoted comma primary");
    $test_utils:assert_true(equal(this:parse_name_aliases("  "), {"", {}}), "blank spec");
    return true;
  endmethod

  method test_time_article_and_curie_helpers owner: HACKER
    "Cover numeric, time, article, and CURIE helpers.";
    $test_utils:assert_true("  -42 ":is_numeric(), "is_numeric accepts signed integers with whitespace");
    $test_utils:assert_type("42":is_numeric(), TYPE_BOOL, "Numeric success is boolean");
    $test_utils:assert_type("abc":is_numeric(), TYPE_BOOL, "Numeric failure is boolean");
    $test_utils:assert_false("4.2":is_numeric(), "is_numeric rejects decimals");
    $test_utils:assert_false("abc":is_numeric(), "is_numeric rejects text");
    $test_utils:assert_true(equal(this:from_seconds(59), "59 seconds"), "from_seconds seconds");
    $test_utils:assert_true(equal(this:from_seconds(60), "a minute"), "from_seconds minute boundary");
    $test_utils:assert_true(equal(this:from_seconds(120), "2 minutes"), "from_seconds plural minutes");
    $test_utils:assert_true(equal(this:from_seconds(3600), "an hour"), "from_seconds hour boundary");
    $test_utils:assert_true(equal(this:from_seconds(7200), "2 hours"), "from_seconds plural hours");
    $test_utils:assert_true(equal(this:from_seconds(86400), "a day"), "from_seconds day boundary");
    next_run = "00:00:00":parse_time_of_day();
    $test_utils:assert_type(next_run, TYPE_INT, "parse_time_of_day returns a timestamp");
    $test_utils:assert_true(next_run > time(), "parse_time_of_day returns a future timestamp");
    $test_utils:assert_raises(E_INVARG, "25:00:00", "parse_time_of_day", {}, "parse_time_of_day rejects invalid hours");
    $test_utils:assert_raises(E_INVARG, "12:60:00", "parse_time_of_day", {}, "parse_time_of_day rejects invalid minutes");
    $test_utils:assert_raises(E_INVARG, "12:30", "parse_time_of_day", {}, "parse_time_of_day rejects missing seconds");
    $test_utils:assert_true(equal("apple":indefinite_article(), "an"), "indefinite_article vowel");
    $test_utils:assert_true(equal("cowbell":indefinite_article(), "a"), "indefinite_article consonant");
    $test_utils:assert_true(equal("apple":with_indefinite_article(), "an apple"), "with_indefinite_article vowel");
    $test_utils:assert_true(equal("cowbell":with_indefinite_article(), "a cowbell"), "with_indefinite_article consonant");
    $test_utils:assert_true(equal("oid:1":parse_curie(), #1), "parse_curie oid");
    $test_utils:assert_true(equal("oid:abc":parse_curie(), false), "parse_curie rejects invalid oid");
    $test_utils:assert_true(equal("sysobj:root":parse_curie(), $root), "parse_curie sysobj");
    $test_utils:assert_true(equal("sysobj:nonexistent_property":parse_curie(), false), "parse_curie rejects invalid sysobj");
    $test_utils:assert_true(equal("match(\"anything\")":parse_curie(#-1), false), "parse_curie rejects invalid match context");
    $test_utils:assert_true(equal("invalid":parse_curie(), false), "parse_curie rejects unknown prefix");
    return true;
  endmethod
  method test_split_boundaries owner: HACKER
    "Splitting preserves empty fields and applies the selected case policy at every length.";
    for example in ({{"", ",", {""}}, {"abc", "", {"abc"}}, {"", "", {""}},
        {",a,,b,", ",", {"", "a", "", "b", ""}},
        {"::a::::b::", "::", {"", "a", "", "b", ""}},
        {"AxB", "X", {"AxB"}}, {"AxyB", "XY", {"AxyB"}}})
      const {source, delimiter, expected} = example;
      $test_utils:assert_true(equal(source:split(delimiter), expected), "Exact split");
      $test_utils:assert_true(equal(source:to_list(delimiter, true), expected), "Exact to_list");
    endfor
    for delimiter in ({"X", "XY"})
      const source = "A" + delimiter:lowercase() + "B";
      $test_utils:assert_true(equal(source:split(delimiter, false), {"A", "B"}),
        "Case-insensitive split");
      $test_utils:assert_true(equal(source:to_list(delimiter), {"A", "B"}),
        "Legacy to_list matching");
      $test_utils:assert_true(equal(source:replace_all(delimiter, "_"), source),
        "Exact replacement preserves unmatched case");
      $test_utils:assert_true(equal(source:replace_all(delimiter, "_", false), "A_B"),
        "Case-insensitive replacement");
    endfor
    return true;
  endmethod

  method test_padding_boundaries owner: HACKER
    "Padding preserves fill alignment and rejects unusable fill strings.";
    $test_utils:assert_true(equal(this:space(5, "ab"), "ababa"), "Left-aligned fill");
    $test_utils:assert_true(equal(this:space(-5, "ab"), "babab"), "Right-aligned fill");
    $test_utils:assert_true(equal(this:space(0, ""), ""), "Zero width needs no fill");
    $test_utils:assert_true(equal(this:space("abc", "."), "..."), "String width");
    $test_utils:assert_true(equal("x":center(6, "ab", "cd"), "abxdcd"),
      "Center anchors the right fill");
    $test_utils:assert_true(equal("long":pad_right(2, ""), "long"), "No padding required");
    $test_utils:assert_raises(E_INVARG, this, "space", {1, ""}, "Reject empty fill");
    $test_utils:assert_raises(E_INVARG, this, "space", {100001}, "Reject excessive width");
    $test_utils:assert_raises(E_INVARG, "x", "pad_left", {3, ""}, "Left padding validates fill");
    $test_utils:assert_raises(E_INVARG, "x", "pad_right", {3, ""}, "Right padding validates fill");
    return true;
  endmethod
endobject
