object SUB_UTILS [
  import_export_id -> "sub_utils",
  import_export_hierarchy -> {"events"}
]
  name: "Substitution Utilities"
  parent: ROOT
  owner: HACKER
  readable: true

  property article_patterns (owner: HACKER, flags: "rc") = {
    {"a ", 3, "a"},
    {"an ", 4, "a"},
    {"the ", 5, "the"},
    {"A ", 3, "ac"},
    {"An ", 4, "ac"},
    {"The ", 5, "thec"}
  };
  property pronoun_map (owner: HACKER, flags: "rc") = [
    "object" -> "o",
    "pos_adj" -> "p",
    "pos_noun" -> "q",
    "reflexive" -> "r",
    "subject" -> "s"
  ];
  property token_map (owner: HACKER, flags: "rc") = [
    "be" -> {"verb_be", {}},
    "be_dobj" -> {"verb_be_dobj", {}},
    "be_iobj" -> {"verb_be_iobj", {}},
    "d" -> {"d", {}},
    "dc" -> {"dc", {}},
    "have" -> {"verb_have", {}},
    "have_dobj" -> {"verb_have_dobj", {}},
    "have_iobj" -> {"verb_have_iobj", {}},
    "i" -> {"i", {}},
    "ic" -> {"ic", {}},
    "l" -> {"l", {}},
    "lc" -> {"lc", {}},
    "look" -> {"verb_look", {}},
    "look_dobj" -> {"verb_look_dobj", {}},
    "look_iobj" -> {"verb_look_iobj", {}},
    "n" -> {"n", {}},
    "nc" -> {"nc", {}},
    "o" -> {"o", {}},
    "o_dobj" -> {"o_dobj", {}},
    "o_iobj" -> {"o_iobj", {}},
    "oc" -> {"oc", {}},
    "oc_dobj" -> {"oc_dobj", {}},
    "oc_iobj" -> {"oc_iobj", {}},
    "p" -> {"p", {}},
    "p_dobj" -> {"p_dobj", {}},
    "p_iobj" -> {"p_iobj", {}},
    "pc" -> {"pc", {}},
    "pc_dobj" -> {"pc_dobj", {}},
    "pc_iobj" -> {"pc_iobj", {}},
    "q" -> {"q", {}},
    "q_dobj" -> {"q_dobj", {}},
    "q_iobj" -> {"q_iobj", {}},
    "qc" -> {"qc", {}},
    "qc_dobj" -> {"qc_dobj", {}},
    "qc_iobj" -> {"qc_iobj", {}},
    "r" -> {"r", {}},
    "r_dobj" -> {"r_dobj", {}},
    "r_iobj" -> {"r_iobj", {}},
    "rc" -> {"rc", {}},
    "rc_dobj" -> {"rc_dobj", {}},
    "rc_iobj" -> {"rc_iobj", {}},
    "s" -> {"s", {}},
    "s_dobj" -> {"s_dobj", {}},
    "s_iobj" -> {"s_iobj", {}},
    "sc" -> {"sc", {}},
    "sc_dobj" -> {"sc_dobj", {}},
    "sc_iobj" -> {"sc_iobj", {}},
    "t" -> {"t", {}},
    "tc" -> {"tc", {}}
  ];
  property typemap (owner: HACKER, flags: "rc") = [
    'actor -> "n",
    'this -> "t",
    'dobj -> "d",
    'iobj -> "i",
    'object -> "o",
    'verb_be -> "be",
    'verb_have -> "have",
    'location -> "l",
    'subject -> "s",
    'reflexive -> "r",
    'pos_adj -> "p",
    'pos_noun -> "q",
    'verb_look -> "look"
  ];
  property verb_map (owner: HACKER, flags: "rc") = ["verb_be" -> "be", "verb_have" -> "have", "verb_look" -> "look"];

  override description (owner: HACKER, flags: "rc") = "Compiler and utilities for $sub template language.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# $sub_utils - Template Compiler",
    "",
    "## Overview",
    "",
    "Compiles human-readable template strings into $sub flyweight content lists,",
    "and can decompile back to templates. Useful for builders creating passage messages",
    "and event content.",
    "",
    "## Template Syntax",
    "",
    "Templates are strings with `{...}` tokens that get replaced with $sub flyweights.",
    "",
    "### Bindings",
    "",
    "```",
    "{actor}       # Actor name (from context)",
    "{direction}   # Any binding name from context",
    "{passage_desc}",
    "```",
    "",
    "### Name Substitutions",
    "",
    "```",
    "{n}   # Actor name",
    "{nc}  # Capitalized actor name",
    "{d}   # Direct object",
    "{i}   # Indirect object",
    "{l}   # Location",
    "{t}   # This object",
    "```",
    "",
    "Add `c` for capitalized: `{nc}`, `{dc}`, etc.",
    "",
    "### Pronouns",
    "",
    "```",
    "{s}       # Actor subject pronoun (he/she/they)",
    "{o}       # Actor object pronoun (him/her/them)",
    "{p}       # Actor possessive adjective (his/her/their)",
    "{q}       # Actor possessive noun (his/hers/theirs)",
    "{r}       # Actor reflexive (himself/herself/themself)",
    "{s_dobj}  # Direct object pronouns (same suffixes available)",
    "{s_iobj}  # Indirect object pronouns",
    "```",
    "",
    "### Verb Conjugation",
    "",
    "```",
    "{be}         # \"are\" / \"is\"",
    "{have}       # \"have\" / \"has\"",
    "{look}       # \"look\" / \"looks\"",
    "{be_dobj}    # Conjugation for direct object",
    "{be_iobj}    # Conjugation for indirect object",
    "```",
    "",
    "### Articles",
    "",
    "```",
    "{a direction}    # Indefinite article: \"a\" or \"an\"",
    "{the direction}  # Definite article: \"the\"",
    "{a d}            # Indefinite article for direct object",
    "{the dc}         # Definite article for capitalized direct object",
    "{a i}            # Indefinite article for indirect object",
    "{The i}          # Capitalized article for indirect object",
    "```",
    "",
    "Articles can use any binding name or substitution abbreviation:",
    "- `d` / `dc` = direct object",
    "- `i` / `ic` = indirect object",
    "- `l` / `lc` = location",
    "- `t` / `tc` = this object",
    "- `n` / `nc` = actor",
    "",
    "Articles check the binding's is_proper_noun() and is_plural() properties.",
    "Returns empty if proper noun (a) or if proper noun (the).",
    "",
    "### Self-Alternation",
    "",
    "```",
    "{feel|feels}     # \"feel\" to actor, \"feels\" to others",
    "{your|their}     # \"your\" to actor, \"their\" to others",
    "```",
    "",
    "Can be nested with other substitutions:",
    "```",
    "{tired|exhausted} {get|gets} you down.",
    "# Actor: \"tired get you down.\"",
    "# Others: \"exhausted gets you down.\"",
    "```",
    "",
    "## Functions",
    "",
    "### compile(template_string) \u2192 list",
    "",
    "Parses a template string and returns a list of strings and $sub flyweights.",
    "",
    "### decompile(content_list) \u2192 string",
    "",
    "Reconstructs a template string from a compiled content list.",
    "",
    "## Examples",
    "",
    "```moo",
    "template = \"{nc} heads {the direction}.\";",
    "content = $sub_utils:compile(template);",
    "# Returns: {",
    "#   $sub:nc(),",
    "#   \" heads \",",
    "#   $sub:the('direction),",
    "#   \".\"",
    "# }",
    "```",
    ""
  };

  method compile owner: HACKER
    "Parse a bounded template into strings and substitution tokens.";
    const {template} = args;
    typeof(template) != TYPE_STR && raise(E_TYPE, "template must be string");
    const len = length(template);
    len > 65536 && raise(E_INVARG, "Template exceeds 65536 characters.");
    let content = {};
    let pos = 1;
    let tokens = 0;
    while (pos <= len)
      const token_start = index(template, "{", 0, pos - 1);
      if (!token_start)
        content = {@content, template[pos..$]};
        break;
      endif
      if (token_start > pos)
        content = {@content, template[pos..token_start - 1]};
      endif
      let depth = 1;
      let scan_pos = token_start + 1;
      let token_end = 0;
      tokens = tokens + 1;
      tokens > 2048 && raise(E_INVARG, "Template exceeds 2048 tokens.");
      while (scan_pos <= len)
        const next_open = index(template, "{", 0, scan_pos - 1);
        const next_close = index(template, "}", 0, scan_pos - 1);
        if (!next_close)
          break;
        endif
        if (next_open && next_open < next_close)
          depth = depth + 1;
          depth > 16 && raise(E_INVARG, "Template nesting exceeds 16 levels.");
          tokens = tokens + 1;
          tokens > 2048 && raise(E_INVARG, "Template exceeds 2048 tokens.");
          scan_pos = next_open + 1;
        else
          depth = depth - 1;
          if (depth == 0)
            token_end = next_close;
            break;
          endif
          scan_pos = next_close + 1;
        endif
      endwhile
      if (!token_end)
        raise(E_INVARG, "Unclosed brace in template at position " + tostr(token_start));
      endif
      const token = template[token_start + 1..token_end - 1]:trim();
      !token && raise(E_INVARG, "Empty substitution token.");
      content = {@content, this:_parse_token(token)};
      pos = token_end + 1;
    endwhile
    return content;
  endmethod

  method _parse_token owner: HACKER
    "Parse a single {token} and return corresponding $sub flyweight.";
    {token_content} = args;
    token_content = token_content:trim();
    "Self-alternation (contains |)";
    pipe_index = index(token_content, "|");
    if (pipe_index)
      parts = token_content:split("|");
      length(parts) != 2 && raise(E_INVARG, "Self-alternation must have exactly 2 parts: " + token_content);
      return $sub:self_alt(parts[1]:trim(), parts[2]:trim());
    endif
    "Articles from property";
    for pattern in (this.article_patterns)
      {prefix, skip, method} = pattern;
      if (token_content:starts_with(prefix))
        binding_name = tosym(token_content[skip..$]:trim());
        return $sub:(method)(binding_name);
      endif
    endfor
    "Direct dispatch from property";
    if (maphaskey(this.token_map, token_content))
      {method, method_args} = this.token_map[token_content];
      return $sub:(method)(@method_args);
    endif
    "Fallback: treat as binding";
    return $sub:binding(tosym(token_content));
  endmethod

  method decompile owner: HACKER
    "Reconstruct template string from compiled content list.";
    {content} = args;
    typeof(content) != TYPE_LIST && raise(E_TYPE, "content must be list");
    result = "";
    for item in (content)
      result = result + (typeof(item) == TYPE_STR ? item | typeof(item) == TYPE_FLYWEIGHT ? this:_reconstruct_token(item) | tostr(item));
    endfor
    return result;
  endmethod

  method _reconstruct_token owner: HACKER
    "Reconstruct {token} syntax from a flyweight.";
    {fw} = args;
    typeof(fw) != TYPE_FLYWEIGHT && raise(E_TYPE, "must be flyweight");
    token_type = fw.type;
    !token_type && return "{?}";
    capitalize = `fw.capitalize ! E_PROPNF => false';
    token_type == 'self_alt && return "{" + tostr(fw.for_self) + "|" + tostr(fw.for_others) + "}";
    if (token_type == 'article_a)
      prefix = capitalize ? "A" | "a";
      return "{" + prefix + " " + tostr(fw.binding_name) + "}";
    endif
    if (token_type == 'article_the)
      prefix = capitalize ? "The" | "the";
      return "{" + prefix + " " + tostr(fw.binding_name) + "}";
    endif
    if (token_type == 'binding)
      name_str = tostr(fw.binding_name);
      return "{" + (capitalize ? name_str:capitalize() | name_str) + "}";
    endif
    "Map type symbols back to token names";
    if (maphaskey(this.typemap, token_type))
      base = this.typemap[token_type];
      return "{" + base + (capitalize ? "c" | "") + "}";
    endif
    "Object-specific versions (dobj_*, iobj_*)";
    token_str = tostr(token_type);
    if (token_str:starts_with("dobj_"))
      base = token_str[6..$];
      maphaskey(this.verb_map, base) && return "{" + this.verb_map[base] + "_dobj}";
      suffix = capitalize ? "c_dobj" | "_dobj";
      maphaskey(this.pronoun_map, base) && return "{" + this.pronoun_map[base] + suffix + "}";
    endif
    if (token_str:starts_with("iobj_"))
      base = token_str[6..$];
      maphaskey(this.verb_map, base) && return "{" + this.verb_map[base] + "_iobj}";
      suffix = capitalize ? "c_iobj" | "_iobj";
      maphaskey(this.pronoun_map, base) && return "{" + this.pronoun_map[base] + suffix + "}";
    endif
    return "{?" + token_str + "?}";
  endmethod

  method test_compile_simple_binding owner: HACKER
    "Test compile with simple binding.";
    result = this:compile("You see {actor}.");
    $test_utils:assert_eq(length(result), 3, "simple binding should compile to three parts");
    $test_utils:assert_eq(result[1], "You see ", "simple binding prefix");
    $test_utils:assert_type(result[2], TYPE_FLYWEIGHT, "simple binding token should be a flyweight");
    $test_utils:assert_eq(result[2].type, 'binding, "simple binding token type");
    $test_utils:assert_eq(result[3], ".", "simple binding suffix");
    return true;
  endmethod

  method test_compile_article owner: HACKER
    "Test compile with article.";
    result = this:compile("{the direction}");
    $test_utils:assert_eq(length(result), 1, "article should compile to one token");
    $test_utils:assert_type(result[1], TYPE_FLYWEIGHT, "article token should be a flyweight");
    $test_utils:assert_eq(result[1].type, 'article_the, "article token type");
    $test_utils:assert_eq(result[1].binding_name, 'direction, "article binding name");
    return true;
  endmethod

  method test_compile_self_alt owner: HACKER
    "Test compile with self-alternation.";
    result = this:compile("{tired|exhausted}");
    $test_utils:assert_eq(length(result), 1, "self-alt should compile to one token");
    $test_utils:assert_type(result[1], TYPE_FLYWEIGHT, "self-alt token should be a flyweight");
    $test_utils:assert_eq(result[1].type, 'self_alt, "self-alt token type");
    $test_utils:assert_eq(result[1].for_self, "tired", "self-alt self text");
    $test_utils:assert_eq(result[1].for_others, "exhausted", "self-alt other text");
    return true;
  endmethod

  method test_compile_mixed owner: HACKER
    "Test compile with mixed content.";
    result = this:compile("{nc} heads {the direction}.");
    $test_utils:assert_eq(length(result), 4, "mixed template should compile to four parts");
    $test_utils:assert_type(result[1], TYPE_FLYWEIGHT, "actor token should be a flyweight");
    $test_utils:assert_eq(result[2], " heads ", "mixed template infix");
    $test_utils:assert_type(result[3], TYPE_FLYWEIGHT, "article token should be a flyweight");
    $test_utils:assert_eq(result[4], ".", "mixed template suffix");
    return true;
  endmethod

  method test_decompile_simple owner: HACKER
    "Test decompile reconstructs template.";
    original = "You see {actor}.";
    compiled = this:compile(original);
    decompiled = this:decompile(compiled);
    $test_utils:assert_eq(decompiled, original, "decompile should reconstruct simple binding template");
    return true;
  endmethod

  method test_decompile_nc_d owner: ARCH_WIZARD
    "Test decompile with nc and d substitutions.";
    original = "{nc} dropped {d}.";
    compiled = this:compile(original);
    decompiled = this:decompile(compiled);
    $test_utils:assert_eq(decompiled, original, "decompile should reconstruct name and dobj substitutions");
    return true;
  endmethod

  verb test_compile_nested (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Test compilation of nested braces.";
    template = "Look at {the {adj} key}.";
    result = this:compile(template);
    "Expected: 'Look at ', FW(article_the, binding='{adj} key'), '.'";
    "Note: The current parser treats the inner content as a raw string binding name because _parse_token doesnt recursively compile.";
    "However, the key improvement is that it DOESNT choke on the inner braces.";
    $test_utils:assert_eq(length(result), 3, "nested template should compile to three parts");
    $test_utils:assert_eq(result[1], "Look at ", "nested template prefix");
    $test_utils:assert_type(result[2], TYPE_FLYWEIGHT, "nested article token should be a flyweight");
    "The binding name should capture the full inner content including braces";
    "If the parser stopped at the first }, it would be '{adj'";
    binding = tostr(result[2].binding_name);
    $test_utils:assert_eq(binding, "{adj} key", "nested binding should preserve inner braces");
    return true;
  endverb

  verb test_compile_properties (this none none) owner: ARCH_WIZARD flags: "rxd"
    "Test property-based map lookups.";
    "Test direct token map";
    res1 = this:compile("You are {n}.");
    $test_utils:assert_eq(res1[2].type, 'actor, "{n} should map to actor substitution");
    "Test verb map lookup via _parse_token logic";
    res2 = this:compile("Use {be}.");
    $test_utils:assert_eq(res2[2].type, 'verb_be, "{be} should map to verb_be substitution");
    "Test specific object suffix logic (dobj_*)";
    res3 = this:compile("It is {be_dobj}.");
    "Note: $sub:verb_be returns 'dobj_verb_be' when called as verb_be_dobj";
    $test_utils:assert_eq(res3[2].type, 'dobj_verb_be, "{be_dobj} should map to dobj verb substitution");
    return true;
  endverb

  method test_compile_and_decompile_errors owner: HACKER
    "Test compiler and decompiler input validation.";
    $test_utils:assert_raises(E_TYPE, this, "compile", {123}, "compile should reject non-string templates");
    $test_utils:assert_raises(E_INVARG, this, "compile", {"Broken {template"}, "compile should reject unclosed braces");
    $test_utils:assert_raises(E_TYPE, this, "decompile", {"not a list"}, "decompile should reject non-list content");
    $test_utils:assert_raises(E_INVARG, this, "_parse_token", {"a|b|c"}, "self alternation should have exactly two parts");
    return true;
  endmethod

  method test_compile_bounds owner: HACKER
    "Malformed and oversized messages fail without exhausting a task.";
    for template in ({"{", "hello{", "x {", "{{", "{}", "{   }"})
      $test_utils:assert_raises(E_INVARG, this, "compile", {template}, template);
    endfor
    $test_utils:assert_raises(E_INVARG, this, "compile", {"x":repeat(65537)}, "source limit");
    $test_utils:assert_raises(E_INVARG, this, "compile", {"{n}":repeat(2049)}, "token limit");
    $test_utils:assert_raises(E_INVARG, this, "compile", {"{":repeat(17) + "n" + "}":repeat(17)}, "nesting limit");
    $test_utils:assert_eq(this:compile(""), {}, "empty messages are valid");
    $test_utils:assert_eq(this:compile("plain text"), {"plain text"}, "literal messages are valid");
    return true;
  endmethod

  method test_decompile_object_specific_tokens owner: HACKER
    "Test decompile reconstructs object-specific pronoun and verb tokens.";
    $test_utils:assert_eq(this:decompile({$sub:sc_dobj()}), "{sc_dobj}", "dobj subject pronoun should decompile");
    $test_utils:assert_eq(this:decompile({$sub:pc_iobj()}), "{pc_iobj}", "iobj possessive pronoun should decompile");
    $test_utils:assert_eq(this:decompile({$sub:verb_have_iobj()}), "{have_iobj}", "iobj verb token should decompile");
    $test_utils:assert_eq(this:decompile({$sub:thec('d)}), "{The d}", "capitalized article should decompile");
    return true;
  endmethod
endobject
