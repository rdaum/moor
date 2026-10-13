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

  override description (owner: HACKER, flags: "rc") = "Compiler and utilities for $sub template language.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Message templates",
    "",
    "A message template describes one event from each participant's point of view.",
    "For example, {nc} {wave|waves}. gives the actor \"You wave.\" and observers \"Alex waves.\"",
    "Use @set-message OBJECT.PROPERTY TEMPLATE to edit a message property.",
    "Use @messages OBJECT to inspect its messages. Commands require authoring permission.",
    "",
    "## Names and pronouns",
    "",
    "{n} selects the actor, {d} the direct object, {i} the indirect object,",
    "{t} this object, and {l} the event's location. Add c to capitalize a name: {nc}.",
    "Names become \"you\" when they refer to the viewer.",
    "",
    "{s}, {o}, {p}, {q}, and {r} select subject, object, possessive adjective,",
    "possessive noun, and reflexive pronouns. For they/them: they, them, their, theirs, themselves.",
    "Add c to capitalize: {sc}. Add _dobj or _iobj to select another participant: {sc_dobj}.",
    "{pronoun recipient subject} selects a named participant. Other forms: object, pos_adj, pos_noun, reflexive.",
    "{be}, {have}, and {look} agree with the actor's pronouns.",
    "Their _dobj and _iobj forms select those participants' pronouns.",
    "A name can need a different verb: \"Alex is\" but \"They are.\"",
    "",
    "## Explicit agreement",
    "",
    "{nc} {agree actor name:are|is} ready. gives Alex is ready. or You are ready.",
    "{sc} {agree actor pronoun:wave|waves}. gives They wave. or You wave.",
    "Use name when the sentence names its subject; use pronoun when it uses a subject pronoun.",
    "Replace actor with dobj, iobj, this, location, or a named object binding.",
    "The first verb form is plural (including you); the second is singular.",
    "Both forms are literal text. Name number and pronoun agreement are independent.",
    "",
    "## Articles and bindings",
    "",
    "{a d} inserts an object name with a/an. {the d} inserts it with the.",
    "{A d} and {The d} capitalize the phrase; {the dc} capitalizes just the name.",
    "These tokens include the name. Do not add another {d} after them.",
    "Proper names and \"you\" omit articles. Plurals and uncountable nouns omit a/an.",
    "A thing's indefinite_article property can override the spelling-based guess with \"a\" or \"an\".",
    "",
    "Other identifiers select named event bindings: {direction}, {from_room}, or {relic}.",
    "Names start with a letter or underscore, followed by letters, digits, underscores, or hyphens.",
    "Use @message-bindings OBJECT.PROPERTY to learn which bindings this message supplies.",
    "Message editors reject unavailable bindings, identify their positions, and suggest similar names.",
    "Direct compile(text) checks syntax only; compile(text, declarations) also checks bindings.",
    "Articles require object bindings. Use {direction} for a direction supplied as text.",
    "{bind s} selects a binding literally named s instead of the subject pronoun token.",
    "{content detail} inserts an explicitly supplied content binding.",
    "Binding text stays literal: braces, HTML, and Djot in that text are not interpreted.",
    "",
    "## Alternatives, capitalization, and literal punctuation",
    "",
    "{left|right} selects left for the actor and right for everyone else.",
    "It selects a perspective, not grammatical agreement. Both branches preserve their whitespace.",
    "{self:left|right} explicitly selects this operation; decompile uses this spelling.",
    "Use the explicit spelling if the left branch starts with the reserved agree or self: prefix.",
    "Branches may contain substitutions or further alternatives:",
    "",
    "{You see {d}|{nc} sees {d}}",
    "",
    "Only the selected branch resolves. An empty branch inserts nothing.",
    "Use {cap CONTENT} to capitalize the first character of resolved content:",
    "{cap {direction}} or {cap {you|they}}.",
    "",
    "Quote literal punctuation with backslash: \\{, \\}, \\|, and \\\\.",
    "A pipe outside braces can also appear literally. Other backslashes remain unchanged.",
    "Unclosed braces, stray closing braces, and empty tokens are errors.",
    "",
    "## Programming interface",
    "",
    "$sub_utils:compile(text) returns a list of literal strings and $sub values.",
    "Pass the list into an event, then resolve that event for each viewer.",
    "$sub_utils:decompile(content) returns an editable template with the same meaning.",
    "It supports strings, substitution tokens, and nested lists of those values.",
    "Other content raises an error instead of being converted to misleading text.",
    "Use an explicit content binding for formatted or computed content.",
    "",
    "Templates allow up to 65536 characters, 2048 tokens, and 16 brace levels.",
    "The scanner also limits text and delimiter pieces to 8192.",
    "Document resolution has separate limits and checks the caller's access permissions."
  };

  method compile owner: HACKER
    "Compile nested substitutions, preserving literal text and branch whitespace.";
    "Backslash quotes braces, pipes, and backslashes. Other backslashes stay literal.";
    const {template, ?bindings = false} = args;
    typeof(template) == TYPE_STR || raise(E_TYPE, "template must be string");
    (typeof(bindings) == TYPE_MAP || equal(bindings, false)) || raise(E_TYPE);
    length(template) <= 65536 || raise(E_INVARG, "Template exceeds 65536 characters.");
    const lexemes = pcre_match(template, "\\\\[{}|\\\\]|[{}|]|[^{}|\\\\]+|\\\\", true, true);
    length(lexemes) <= 8192 || raise(E_INVARG, "Template exceeds 8192 text and delimiter pieces.");
    let stack = {};
    let parts = {};
    let left = false;
    let tokens = 0;
    let opening = 0;
    for lexeme in (lexemes)
      let text = lexeme["0"]["match"];
      const position = lexeme["0"]["position"][1];
      if (text == "{")
        tokens = tokens + 1;
        tokens <= 2048 || raise(E_INVARG, "Template exceeds 2048 tokens.");
        length(stack) < 16 || raise(E_INVARG, "Template nesting exceeds 16 levels.");
        stack = {@stack, {parts, left, opening}};
        opening = position;
        parts = {};
        left = false;
      elseif (text == "}")
        length(stack) || raise(E_INVARG, "Unexpected closing brace in template.");
        let token;
        try
          token = this:_finish_token(parts, left);
          if (typeof(bindings) == TYPE_MAP)
            this:_check_token_binding(token, bindings);
          endif
        except error (E_INVARG)
          raise(E_INVARG, "At character " + tostr(opening) + ": " + error[2]);
        endtry
        {parts, left, opening} = stack[$];
        stack = stack[1..$ - 1];
        parts = {@parts, token};
      elseif (text == "|" && length(stack))
        typeof(left) != TYPE_LIST || raise(E_INVARG, "Alternation requires exactly two branches.");
        left = parts;
        parts = {};
      else
        if (length(text) == 2 && text[1] == "\\")
          text = text[2];
        endif
        if (length(parts) && typeof(parts[$]) == TYPE_STR)
          parts[$] = parts[$] + text;
        else
          parts = {@parts, text};
        endif
      endif
    endfor
    !stack || raise(E_INVARG, "Unclosed brace in template.");
    return parts;
  endmethod

  method _check_token_binding owner: HACKER
    "Check one compiled token against bindings supplied by this message's producer.";
    const {token, bindings} = args;
    const kind = token.type;
    kind == 'capitalize && return;
    let name = 'actor;
    let allowed = {'object};
    if (kind in {'binding, 'content_binding, 'article_a, 'article_the, 'agree, 'pronoun})
      name = token.binding_name;
      if (kind == 'binding)
        allowed = {'object, 'text};
      elseif (kind == 'content_binding)
        allowed = {'content};
      endif
    elseif (kind in {'actor, 'dobj, 'iobj, 'this, 'location})
      name = kind;
    elseif (index(tostr(kind), "dobj_", 1) == 1)
      name = 'dobj;
    elseif (index(tostr(kind), "iobj_", 1) == 1)
      name = 'iobj;
    endif
    name = $document_resolver:binding_name(name);
    if (!maphaskey(bindings, name))
      const candidates = {tostr(key) for key in (mapkeys(bindings))};
      const match = complex_match(tostr(name), candidates, 0.5);
      let message = "This message does not supply {" + tostr(name) + "}.";
      if (typeof(match) == TYPE_STR)
        message = message + " Did you mean {" + match + "}?";
      endif
      raise(E_INVARG, message);
    endif
    bindings[name]['kind] in allowed || raise(E_INVARG,
      "{" + tostr(name) + "} supplies " + tostr(bindings[name]['kind])
      + "; this token needs " + {tostr(value) for value in (allowed)}:join(" or ") + ".");
  endmethod

  method standard_bindings owner: HACKER
    "Describe the actor and captured location shared by event producers, including preview examples.";
    const {actor_description} = args;
    return ['actor -> ['kind -> 'object, 'description -> actor_description, 'example -> 'actor],
      'location -> ['kind -> 'object, 'description -> "The actor's location when the event starts.",
        'example -> 'location]];
  endmethod

  method _finish_token owner: HACKER
    "Build one token after its nested tokens have been compiled.";
    const {parts, left} = args;
    if (typeof(left) == TYPE_LIST)
      if (length(left) && typeof(left[1]) == TYPE_STR && index(left[1], "self:", 1) == 1)
        return $sub:self_alt({left[1][6..$], @left[2..$]}, parts);
      endif
      if (length(left) && typeof(left[1]) == TYPE_STR && index(left[1], "agree ", 1) == 1)
        const header = pcre_match(left[1], "^agree ([A-Za-z_][A-Za-z0-9_-]*) (name|pronoun):", true, false);
        (length(header) && length(left) == 1 && length(parts) == 1 && typeof(parts[1]) == TYPE_STR)
          || raise(E_INVARG, "Use {agree BINDING name:plural|singular} or pronoun in place of name.");
        const fields = header[1];
        return $sub:agree(tosym(fields["1"]["match"]), tosym(fields["2"]["match"]),
          left[1][fields["0"]["position"][2] + 1..$], parts[1]);
      endif
      return $sub:self_alt(left, parts);
    endif
    if (length(parts) && typeof(parts[1]) == TYPE_STR && index(parts[1], "cap ", 1) == 1)
      return $sub:capitalize({parts[1][5..$], @parts[2..$]});
    endif
    (length(parts) == 1 && typeof(parts[1]) == TYPE_STR)
      || raise(E_INVARG, "Expected a token name, cap content, or two alternatives.");
    return this:_parse_token(parts[1]);
  endmethod

  method _binding_name owner: HACKER
    "Require a binding identifier with letters, digits, underscores, or hyphens.";
    const {name} = args;
    length(pcre_match(name, "^[A-Za-z_][A-Za-z0-9_-]*$", true, false))
      || raise(E_INVARG, "Invalid binding name: " + name);
    return tosym(name);
  endmethod

  method _parse_token owner: HACKER
    "Parse an atomic token; nesting and alternatives are handled by compile.";
    const {raw} = args;
    const token = raw:trim();
    length(token) || raise(E_INVARG, "Empty substitution token.");
    for pattern in (this.article_patterns)
      const {prefix, skip, method} = pattern;
      if (index(token, prefix, 1) == 1)
        return $sub:(method)(this:_binding_name(token[skip..$]:trim()));
      endif
    endfor
    if (index(token, "bind ", 1) == 1)
      return $sub:binding(this:_binding_name(token[6..$]:trim()));
    elseif (index(token, "content ", 1) == 1)
      return $sub:content_binding(this:_binding_name(token[9..$]:trim()));
    elseif (index(token, "pronoun ", 1) == 1)
      const words = token:words();
      length(words) == 3 || raise(E_INVARG, "Use {pronoun BINDING FORM}.");
      return $sub:pronoun(this:_binding_name(words[2]), tosym(words[3]));
    endif
    if (maphaskey(this.token_map, token))
      const {method, method_args} = this.token_map[token];
      return $sub:(method)(@method_args);
    endif
    return $sub:binding(this:_binding_name(token));
  endmethod

  method decompile owner: HACKER
    "Write an editable template, or reject content the template language cannot represent.";
    const {content} = args;
    typeof(content) == TYPE_LIST || raise(E_TYPE, "content must be list");
    const names = this:_token_names();
    let pending = {{content, 0, false}};
    let result = "";
    let count = 0;
    while (pending)
      const {value, depth, raw} = pending[$];
      pending = pending[1..$ - 1];
      count = count + 1;
      (depth <= 48 && count <= 32768) || raise(E_INVARG, "Content exceeds template limits.");
      if (raw)
        result = result + value;
      elseif (typeof(value) == TYPE_STR)
        result = result + this:_escape(value);
      elseif (typeof(value) == TYPE_LIST)
        length(value) <= 8192 || raise(E_INVARG, "Too many template entries.");
        for i in [1..length(value)]
          pending = {@pending, {value[$ - i + 1], depth + 1, false}};
        endfor
      elseif (typeof(value) == TYPE_FLYWEIGHT && value.delegate == $sub)
        if (value.type == 'capitalize)
          pending = {@pending, {"}", depth, true}, {value.content, depth + 1, false},
            {"{cap ", depth, true}};
        elseif (value.type == 'self_alt)
          const cap = `value.capitalize ! E_PROPNF => false';
          pending = {@pending, {cap ? "}}" | "}", depth, true},
            {value.for_others, depth + 1, false}, {"|", depth, true},
            {value.for_self, depth + 1, false}, {cap ? "{cap {self:" | "{self:", depth, true}};
        else
          result = result + this:_reconstruct_token(value, names);
        endif
      else
        raise(E_INVARG, "This content cannot be represented by a substitution template.");
      endif
      length(result) <= 65536 || raise(E_INVARG, "Template exceeds 65536 characters.");
    endwhile
    "Check the source against the same bounds as hand-written templates.";
    this:compile(result);
    return result;
  endmethod

  method _escape owner: HACKER
    "Quote literal delimiters so substituted content never becomes template syntax.";
    const {text} = args;
    let escaped = strsub(text, "\\", "\\\\", 1);
    for delimiter in ({"{", "}", "|"})
      escaped = strsub(escaped, delimiter, "\\" + delimiter, 1);
    endfor
    return escaped;
  endmethod

  method _token_names owner: HACKER
    "Derive reverse spellings once per decompile from the compiler's vocabulary.";
    let names = [];
    for name in (mapkeys(this.token_map))
      const {method, method_args} = this.token_map[name];
      const token = $sub:(method)(@method_args);
      const capitalize = `token.capitalize ! E_PROPNF => false';
      names[tostr(token.type) + (capitalize ? ":c" | ":")] = name;
    endfor
    return names;
  endmethod

  method _reconstruct_token owner: HACKER
    "Reconstruct a supported substitution without discarding capitalization or nested content.";
    const {fw, names} = args;
    (typeof(fw) == TYPE_FLYWEIGHT && fw.delegate == $sub) || raise(E_TYPE, "Expected a $sub token.");
    const kind = fw.type;
    const capitalize = `fw.capitalize ! E_PROPNF => false';
    let token = "";
    if (kind in {'article_a, 'article_the})
      const prefix = kind == 'article_a ? "a" | "the";
      const name = tostr(fw.binding_name);
      this:_binding_name(name);
      const suffix = `fw.capitalize_binding ! E_PROPNF => false' ? "c" | "";
      if (suffix)
        fw.binding_name in {'n, 'd, 'i, 't, 'l}
          || raise(E_INVARG, "Unsupported capitalized article binding.");
      endif
      return "{" + (capitalize ? prefix:capitalize() | prefix) + " " + name + suffix + "}";
    elseif (kind == 'pronoun)
      const name = tostr(fw.binding_name);
      this:_binding_name(name);
      token = "{pronoun " + name + " " + tostr(fw.form) + "}";
    elseif (kind == 'agree)
      const name = tostr(fw.binding_name);
      this:_binding_name(name);
      token = "{agree " + name + " " + tostr(fw.subject_form) + ":"
        + this:_escape(fw.plural_form) + "|" + this:_escape(fw.singular_form) + "}";
    elseif (kind in {'binding, 'content_binding})
      const name = tostr(fw.binding_name);
      this:_binding_name(name);
      let prefix = kind == 'content_binding ? "content " | "";
      kind == 'binding && maphaskey(this.token_map, name) && (prefix = "bind ");
      token = "{" + prefix + name + "}";
    else
      const key = tostr(kind) + (capitalize ? ":c" | ":");
      if (maphaskey(names, key))
        return "{" + names[key] + "}";
      endif
      length(token) || raise(E_INVARG, "Unknown substitution: " + tostr(kind));
    endif
    return capitalize ? "{cap " + token + "}" | token;
  endmethod

  method test_binding_diagnostics owner: HACKER
    "Editing reports the misspelled binding and its source position before saving.";
    const bindings = $thing:message_bindings("get_msg");
    for example in ({{"Hello {dobbj}.", "At character 7:", "Did you mean {dobj}?"},
      {"é {dobbj}", "At character 3:", "Did you mean {dobj}?"},
      {"{self:ok|{missing}}", "At character 10:", "{missing}"},
      {"{pronoun iobj subject}", "", ""}})
      try
        this:compile(example[1], bindings);
        $test_utils:assert_eq(example[2], "", "unknown bindings must fail during editing");
      except error (E_INVARG)
        $test_utils:assert_true(length(example[2]) > 0, "valid binding compiles");
        $test_utils:assert_true(index(error[2], example[2]) > 0, "error locates the token");
        $test_utils:assert_true(index(error[2], example[3]) > 0, "error explains the binding");
      endtry
    endfor
    const movement = $passage:message_bindings("side_a_leave_msg");
    this:compile("{nc} goes {direction} to {to_room}.", movement);
    $test_utils:assert_raises(E_INVARG, this, "compile", {"{pronoun direction subject}", movement},
      "text bindings cannot supply pronouns");
    $test_utils:assert_raises(E_INVARG, this, "compile", {"{the direction}", movement},
      "text bindings cannot supply articles");
    $test_utils:assert_raises(E_INVARG, this, "compile", {"{content direction}", movement},
      "text bindings cannot supply content");
    $test_utils:assert_raises(E_INVARG, $reaction, "parse_effect", {{'announce, "{i}"}},
      "announce does not supply an indirect object");
    this:compile("literal text", []);
    return true;
  endmethod

  method test_producer_binding_declarations owner: HACKER
    "Each shipped editable template uses only bindings declared by its producer.";
    for producer in ({$thing, $container, $consumable, $food, $drink, $wearable, $note,
      $sittable, $passage, $llm_room_observer})
      for property in (properties(producer))
        if (!tostr(property):ends_with("_msg"))
          continue;
        endif
        const content = producer.(property);
        if (typeof(content) == TYPE_LIST)
          this:compile(this:decompile(content), producer:message_bindings(property));
        endif
      endfor
    endfor
    const child = $food:create(true);
    this:compile("{d}", child:message_bindings("get_msg"));
    this:compile("{d}", child:message_bindings("eat_msg"));
    $test_utils:assert_eq(child:message_bindings("unknown_msg"), [], "unknown producer has no bindings");
    return true;
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
    $test_utils:assert_eq(result[1].for_self, {"tired"}, "self-alt self text");
    $test_utils:assert_eq(result[1].for_others, {"exhausted"}, "self-alt other text");
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

  method test_compile_nested owner: HACKER
    "Nested alternatives resolve only the chosen branches, including their participant tokens.";
    const actor = $thing:create(true);
    actor.name = "alex";
    const item = $thing:create(true);
    item.name = "pebble";
    for example in ({{"{You see {d}|{nc} sees {d}}", "You see pebble", "Alex sees pebble"},
      {"{you {take|takes} {d}|{nc} {take|takes} {d}}", "you take pebble", "Alex takes pebble"},
      {"{ safe |{missing}}", " safe ", false}, {"{|other}", "", "other"}})
      const event = $event:mk_test(actor, @this:compile(example[1])):with_dobj(item);
      $test_utils:assert_eq(strcmp($document:render(event:resolve(actor), 'text_plain)['body],
        example[2]), 0, "actor: " + example[1]);
      if (typeof(example[3]) == TYPE_STR)
        $test_utils:assert_eq(strcmp($document:render(event:resolve(player), 'text_plain)['body],
          example[3]), 0, "observer: " + example[1]);
      endif
    endfor
    for invalid in ({"{the {adj} key}", "{a|b|c}", "stray }", "{unclosed"})
      $test_utils:assert_raises(E_INVARG, this, "compile", {invalid}, invalid);
    endfor
    return true;
  endmethod

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
    $test_utils:assert_raises(E_INVARG, this, "compile", {"|":repeat(8193)}, "delimiter limit");
    const deepest = "{cap ":repeat(15) + "{n}" + "}":repeat(15);
    $test_utils:assert_eq(this:decompile(this:compile(deepest)), deepest, "nesting boundary roundtrip");
    $test_utils:assert_eq(this:compile(""), {}, "empty messages are valid");
    $test_utils:assert_eq(this:compile("plain text"), {"plain text"}, "literal messages are valid");
    return true;
  endmethod

  method test_template_roundtrips owner: HACKER
    "Editing compiled templates preserves both perspectives, literal delimiters, and capitalization.";
    const actor = $thing:create(true);
    actor.name = "éloïse";
    const item = $thing:create(true);
    item.name = "apple";
    const samples = {{"literal {n} | \\ end"}, {"plain ", this:compile("{nc}")[1]},
      {$sub:bindingc('direction)}, {$sub:binding('s)}, {$sub:the('dc)},
      {$sub:self_altc("you", {"they see ", $sub:d()})},
      {$sub:self_alt({$sub:nc(), " | ", $sub:d()}, {$sub:thec('d), " is here"})},
      {$sub:content_binding('detail)}, {$sub:self_alt("agree actor name:are", "is")},
      {$sub:self_alt("self:literal", "other")}};
    for content in (samples)
      const edited = this:compile(this:decompile(content));
      const original = $event:mk_test(actor, @content):with_dobj(item)
        :with_text_binding('direction, "north"):with_text_binding('s, "literal subject")
        :with_content_binding('detail, $format.code:inline("{literal}"));
      const rebuilt = $event:mk_test(actor, @edited):with_dobj(item)
        :with_text_binding('direction, "north"):with_text_binding('s, "literal subject")
        :with_content_binding('detail, $format.code:inline("{literal}"));
      for viewer in ({actor, player})
        for format in ({'text_plain, 'text_html, 'text_djot})
          const expected = $document:render(original:resolve(viewer), format);
          const actual = $document:render(rebuilt:resolve(viewer), format);
          $test_utils:assert_true(equal(actual, expected), "roundtrip: " + this:decompile(content));
        endfor
      endfor
    endfor
    $test_utils:assert_raises(E_INVARG, this, "decompile", {{$format.code:inline("code")}},
      "format nodes need explicit content bindings");
    $test_utils:assert_raises(E_INVARG, this, "decompile", {{123}}, "do not stringify unsupported values");
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
