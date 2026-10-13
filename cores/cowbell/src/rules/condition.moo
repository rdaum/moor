object CONDITION [
  import_export_id -> "condition",
  import_export_hierarchy -> {"rules"}
]
  name: "Conditions"
  parent: ROOT
  owner: HACKER
  readable: true

  method parse owner: HACKER
    "Parse the entire expression into the canonical condition tree, retaining source positions.";
    const {text} = args;
    const tokens = $condition:_tokens(text);
    length(tokens) > 0 || raise(E_INVARG, "Empty condition");
    const {tree, next} = $condition:_parse_or(tokens, 1, 0);
    next > length(tokens) || raise(E_INVARG,
      "Unexpected token at character " + tostr(tokens[next]['start]));
    return tree;
  endmethod

  method _tokens owner: HACKER
    "Tokenize quoted literals, names, punctuation, and comparison operators without evaluation.";
    const {text} = args;
    typeof(text) == TYPE_STR || raise(E_INVARG, "Condition must be text");
    length(text) <= 32768 || raise(E_QUOTA, "Condition exceeds 32768 characters");
    let tokens = {};
    let pos = 1;
    while (pos <= length(text))
      const start = pos;
      const char = text[pos];
      if (char in {" ", "\t", "\r", "\n"})
        pos = pos + 1;
        continue;
      endif
      if (char == "\"")
        pos = pos + 1;
        while (pos <= length(text) && text[pos] != "\"")
          if (text[pos] == "\\")
            pos = pos + 1;
          endif
          pos = pos + 1;
        endwhile
        pos <= length(text) || raise(E_INVARG, "Unterminated string at character " + tostr(start));
        pos = pos + 1;
      elseif (char in {"{", "["})
        let closing = {char == "{" ? "}" | "]"};
        pos = pos + 1;
        let quoted = false;
        while (pos <= length(text) && length(closing) > 0)
          const current = text[pos];
          if (quoted && current == "\\")
            pos = pos + 1;
          elseif (current == "\"")
            quoted = !quoted;
          elseif (!quoted)
            if (current in {"{", "["})
              closing = {@closing, current == "{" ? "}" | "]"};
            elseif (current in {"}", "]"})
              current == closing[$] || raise(E_INVARG, "Mismatched literal at character " + tostr(pos));
              closing = closing[1..$ - 1];
            endif
          endif
          pos = pos + 1;
        endwhile
        length(closing) == 0 || raise(E_INVARG, "Unterminated literal at character " + tostr(start));
      elseif (char in {"(", ")", ",", ":"})
        pos = pos + 1;
      elseif (char in {"=", "!", "<", ">"} || (char == "-" && pos < length(text) && text[pos + 1] == ">"))
        pos = pos + 1;
        if (pos <= length(text) && text[start..pos] in {"==", "!=", "<=", ">=", "->"})
          pos = pos + 1;
        endif
      else
        while (pos <= length(text) && !(text[pos] in {" ", "\t", "\r", "\n", "(", ")", ",", ":", "=", "!", "<", ">"}))
          pos = pos + 1;
        endwhile
      endif
      tokens = {@tokens, ['text -> text[start..pos - 1], 'start -> start, 'end -> pos]};
      length(tokens) <= 4096 || raise(E_QUOTA, "Condition exceeds 4096 tokens");
    endwhile
    return tokens;
  endmethod

  method _expect owner: HACKER
    "Require one token, retaining a source position in syntax errors.";
    const {tokens, pos, expected} = args;
    pos <= length(tokens) || raise(E_INVARG, "Expected " + expected + " at end of condition");
    tokens[pos]['text] == expected || raise(E_INVARG,
      "Expected " + expected + " at character " + tostr(tokens[pos]['start]));
    return pos + 1;
  endmethod

  method _parse_or owner: HACKER
    "Parse OR below AND in precedence. Parenthesized children retain their own trees.";
    let {tokens, pos, depth} = args;
    const start = pos;
    let {tree, next} = $condition:_parse_and(tokens, pos, depth);
    pos = next;
    let children = {tree};
    while (pos <= length(tokens) && tokens[pos]['text] == "OR")
      {tree, next} = $condition:_parse_and(tokens, pos + 1, depth);
      children = {@children, tree};
      pos = next;
    endwhile
    length(children) == 1 && return {children[1], pos};
    return {['kind -> 'any, 'children -> children,
      'source -> ['start -> tokens[start]['start], 'end -> tokens[pos - 1]['end]]], pos};
  endmethod

  method _parse_and owner: HACKER
    "Parse a sequence of terms joined by AND.";
    let {tokens, pos, depth} = args;
    const start = pos;
    let {tree, next} = $condition:_parse_term(tokens, pos, depth);
    pos = next;
    let children = {tree};
    while (pos <= length(tokens) && tokens[pos]['text] == "AND")
      {tree, next} = $condition:_parse_term(tokens, pos + 1, depth);
      children = {@children, tree};
      pos = next;
    endwhile
    length(children) == 1 && return {children[1], pos};
    return {['kind -> 'all, 'children -> children,
      'source -> ['start -> tokens[start]['start], 'end -> tokens[pos - 1]['end]]], pos};
  endmethod

  method _parse_term owner: HACKER
    "Parse a grouped condition, negation, predicate call, or explicit comparison.";
    let {tokens, pos, depth} = args;
    depth <= 16 || raise(E_QUOTA, "Condition nesting exceeds 16 levels");
    pos <= length(tokens) || raise(E_INVARG, "Expected condition at end of input");
    const start = pos;
    const token = tokens[pos]['text];
    let tree;
    if (token == "(")
      const {child, next} = $condition:_parse_or(tokens, pos + 1, depth + 1);
      return {child, $condition:_expect(tokens, next, ")")};
    elseif (token == "NOT")
      const {child, next} = $condition:_parse_term(tokens, pos + 1, depth + 1);
      tree = ['kind -> 'not, 'child -> child];
      pos = next;
    elseif (token in {"TRUE", "FALSE"} && (pos == length(tokens)
      || !(tokens[pos + 1]['text] in {"==", "!=", "<", "<=", ">", ">="})))
      tree = ['kind -> token == "TRUE" ? 'all | 'any, 'children -> {}];
      pos = pos + 1;
    elseif (pos < length(tokens) && tokens[pos + 1]['text] == "(")
      $behavior_schema:_name(tosym(token), "predicate");
      let relation = 0;
      let arguments_pos = pos + 2;
      if (token == "relation")
        arguments_pos <= length(tokens) || raise(E_INVARG, "Expected a relation reference.");
        relation = $condition:_value(tokens[arguments_pos]);
        arguments_pos = arguments_pos + 1;
        if (arguments_pos <= length(tokens) && tokens[arguments_pos]['text] != ")")
          arguments_pos = $condition:_expect(tokens, arguments_pos, ",");
          (arguments_pos <= length(tokens) && tokens[arguments_pos]['text] != ")")
            || raise(E_INVARG, "Expected a field after comma.");
        endif
      endif
      const {arguments, next} = $condition:_named(tokens, arguments_pos, false);
      pos = next;
      let bind = [];
      if (pos <= length(tokens) && tokens[pos]['text] == "->")
        pos = $condition:_expect(tokens, pos + 1, "(");
        const {outputs, after} = $condition:_named(tokens, pos, true);
        bind = outputs;
        pos = after;
      endif
      tree = token == "relation"
        ? ['kind -> 'relation, 'relation -> relation, 'arguments -> arguments, 'bind -> bind]
        | ['kind -> 'predicate, 'name -> tosym(token), 'arguments -> arguments, 'bind -> bind];
    else
      const left = $condition:_value(tokens[pos]);
      pos = pos + 1;
      pos <= length(tokens) || raise(E_INVARG, "Expected comparison operator");
      const operators = ["==" -> 'eq, "!=" -> 'ne, "<" -> 'lt, "<=" -> 'le, ">" -> 'gt, ">=" -> 'ge];
      maphaskey(operators, tokens[pos]['text]) || raise(E_INVARG,
        "Expected comparison at character " + tostr(tokens[pos]['start]));
      const op = operators[tokens[pos]['text]];
      pos = pos + 1;
      pos <= length(tokens) || raise(E_INVARG, "Expected right comparison value");
      tree = ['kind -> 'compare, 'op -> op, 'left -> left, 'right -> $condition:_value(tokens[pos])];
      pos = pos + 1;
    endif
    tree['source] = ['start -> tokens[start]['start], 'end -> tokens[pos - 1]['end]];
    return {tree, pos};
  endmethod

  method _named owner: HACKER
    "Parse named value arguments or output bindings through the closing parenthesis.";
    let {tokens, pos, outputs} = args;
    let fields = [];
    while (pos <= length(tokens) && tokens[pos]['text] != ")")
      const name = tosym(tokens[pos]['text]);
      $behavior_schema:_name(name, "argument");
      !maphaskey(fields, name) || raise(E_INVARG, "Repeated argument " + tostr(name));
      pos = $condition:_expect(tokens, pos + 1, ":");
      pos <= length(tokens) || raise(E_INVARG, "Expected argument value");
      if (outputs)
        const binding = tosym(tokens[pos]['text]);
        $behavior_schema:_name(binding, "output binding");
        fields[name] = binding;
      else
        fields[name] = $condition:_value(tokens[pos]);
      endif
      pos = pos + 1;
      if (pos <= length(tokens) && tokens[pos]['text] == ",")
        pos = pos + 1;
        (pos <= length(tokens) && tokens[pos]['text] != ")")
          || raise(E_INVARG, "Expected argument after comma");
      else
        break;
      endif
    endwhile
    return {fields, $condition:_expect(tokens, pos, ")")};
  endmethod

  method _value owner: HACKER
    "Resolve literal syntax without object matching, or identify an explicit value namespace.";
    const {token} = args;
    const text = token['text];
    const dot = index(text, ".");
    let value;
    if (dot > 1 && text[1..dot - 1] in {"input", "parameter", "state", "binding"})
      const name = tosym(text[dot + 1..$]);
      $behavior_schema:_name(name, "value");
      value = ['kind -> tosym(text[1..dot - 1]), 'name -> name];
    else
      try
        value = ['kind -> 'literal, 'value -> fromliteral(text)];
      except error (E_INVARG, E_TYPE)
        raise(E_INVARG, "Invalid value at character " + tostr(token['start]));
      endtry
    endif
    value['source] = ['start -> token['start], 'end -> token['end]];
    return value;
  endmethod

  method relation_target owner: HACKER
    "Resolve a fixed relation object or an explicit reaction setting. Event values cannot choose storage.";
    const {expression, parameters} = args;
    if (expression['kind] == 'literal)
      return expression['value];
    endif
    expression['kind] == 'parameter || raise(E_INVARG, "Use a relation object or a reaction setting.");
    maphaskey(parameters, expression['name])
      || raise(E_INVARG, "Missing relation setting: " + tostr(expression['name]));
    return parameters[expression['name]];
  endmethod

  method prepare_relations owner: ARCH_WIZARD
    "Capture required field types when authoring; check existing requirements when installing or retargeting.";
    set_task_perms(caller_perms());
    let {tree, ?parameters = [], ?depth = 0} = args;
    depth <= 16 || raise(E_QUOTA, "Condition nesting exceeds 16 levels.");
    if (tree['kind] in {'all, 'any})
      tree['children] = {$condition:prepare_relations(child, parameters, depth + 1)
        for child in (tree['children])};
    elseif (tree['kind] == 'not)
      tree['child] = $condition:prepare_relations(tree['child], parameters, depth + 1);
    elseif (tree['kind] == 'relation)
      const relation = $condition:relation_target(tree['relation], parameters);
      const schema = $relation:schema(relation, `tree['fields] ! E_RANGE => 0');
      tree['fields] = schema['types];
    endif
    return tree;
  endmethod

  method print owner: HACKER
    "Print an unambiguous editable expression. Source positions are recomputed when reparsed.";
    const {tree} = args;
    const kind = tree['kind];
    if (kind in {'all, 'any})
      length(tree['children]) == 0 && return kind == 'all ? "TRUE" | "FALSE";
      const children = {$condition:print(child) for child in (tree['children])};
      return "(" + children:join(kind == 'all ? " AND " | " OR ") + ")";
    elseif (kind == 'not)
      return "NOT (" + $condition:print(tree['child]) + ")";
    elseif (kind == 'compare)
      const operators = ['eq -> "==", 'ne -> "!=", 'lt -> "<", 'le -> "<=", 'gt -> ">", 'ge -> ">="];
      return $condition:print_value(tree['left]) + " " + operators[tree['op]] + " " + $condition:print_value(tree['right]);
    elseif (kind in {'predicate, 'relation})
      const arguments = {tostr(name) + ": " + $condition:print_value(tree['arguments][name])
        for name in (mapkeys(tree['arguments]))};
      const prefix = kind == 'relation
        ? "relation(" + $condition:print_value(tree['relation]) + (length(arguments) > 0 ? ", " | "")
        | tostr(tree['name]) + "(";
      let text = prefix + arguments:join(", ") + ")";
      if (length(tree['bind]) > 0)
        const outputs = {tostr(name) + ": " + tostr(tree['bind][name]) for name in (mapkeys(tree['bind]))};
        text = text + " -> (" + outputs:join(", ") + ")";
      endif
      return text;
    endif
    raise(E_INVARG, "Unknown condition kind");
  endmethod

  method print_value owner: HACKER
    "Print exact literal values or namespace-qualified references.";
    const {value} = args;
    if (value['kind] == 'literal)
      const text = toliteral(value['value]);
      equal(fromliteral(text), value['value]) || raise(E_INVARG, "Literal has no exact text representation");
      return text;
    endif
    return tostr(value['kind]) + "." + tostr(value['name]);
  endmethod

  method resolve owner: HACKER
    "Read a literal or one declared value from the supplied environment. No world callbacks.";
    const {value, environment} = args;
    value['kind] == 'literal && return value['value];
    return environment[value['kind]][value['name]];
  endmethod

  method references owner: HACKER
    "Collect syntax references. Literal values never become variable references.";
    const {tree} = args;
    let result = ['input -> {}, 'parameter -> {}, 'state -> {}, 'binding -> {}];
    let pending = {tree};
    while (length(pending) > 0)
      const node = pending[$];
      pending = pending[1..$ - 1];
      const kind = node['kind];
      if (kind in {'input, 'parameter, 'state, 'binding})
        result[kind] = setadd(result[kind], node['name]);
      elseif (kind in {'all, 'any})
        pending = {@pending, @node['children]};
      elseif (kind == 'not)
        pending = {@pending, node['child]};
      elseif (kind == 'compare)
        pending = {@pending, node['left], node['right]};
      elseif (kind in {'predicate, 'relation})
        pending = {@pending, @mapvalues(node['arguments])};
        kind == 'relation && (pending = {@pending, node['relation]});
      endif
    endwhile
    return result;
  endmethod

  method evaluate owner: ARCH_WIZARD
    "Evaluate a validated condition with an explicit predicate callback under caller authority.";
    set_task_perms(caller_perms());
    const {tree, environment, provider, ?limit = 256} = args;
    (typeof(limit) == TYPE_INT && limit > 0 && limit <= 256) || raise(E_INVARG, "Invalid condition limit");
    return $condition:_evaluate(tree, environment, provider, limit, 0);
  endmethod

  method _evaluate owner: ARCH_WIZARD
    "Compose bounded binding sets. Incomplete searches never establish absence.";
    set_task_perms(caller_perms());
    const {tree, environment, provider, limit, depth} = args;
    depth <= 16 || raise(E_QUOTA, "Condition nesting exceeds 16 levels");
    const kind = tree['kind];
    const initial = environment['binding];
    if (kind == 'not)
      const result = $condition:_evaluate(tree['child], environment, provider, limit, depth + 1);
      length(result['bindings]) > 0 && return ['bindings -> {}, 'complete -> true];
      result['complete] || raise(E_QUOTA, "Negation requires a complete search");
      return ['bindings -> {initial}, 'complete -> true];
    elseif (kind == 'compare)
      const left = $condition:resolve(tree['left], environment);
      const right = $condition:resolve(tree['right], environment);
      const op = tree['op];
      let matches;
      if (op in {'eq, 'ne})
        matches = equal(left, right);
        op == 'ne && (matches = !matches);
      else
        let first = left;
        let second = right;
        if (typeof(left) == TYPE_STR && typeof(right) == TYPE_STR)
          first = strcmp(left, right);
          second = 0;
        endif
        if (op == 'lt)
          matches = first < second;
        elseif (op == 'le)
          matches = first <= second;
        elseif (op == 'gt)
          matches = first > second;
        else
          matches = first >= second;
        endif
      endif
      return ['bindings -> matches ? {initial} | {}, 'complete -> true];
    elseif (kind in {'predicate, 'relation})
      let arguments = [];
      for name in (mapkeys(tree['arguments]))
        arguments[name] = $condition:resolve(tree['arguments][name], environment);
      endfor
      const result = kind == 'relation
        ? $relation:query_rows($condition:resolve(tree['relation], environment), tree['fields], arguments, limit)
        | provider(tree['name], arguments, limit);
      const outputs = mapkeys(tree['bind]);
      const pattern = {{'var, tree['bind][output]} for output in (outputs)};
      const rows = {{row[field] for field in (outputs)} for row in (result['rows])};
      const bindings = term_match(pattern, rows, initial,
        ['case_sensitive -> true, 'dedupe -> true, 'max_solutions -> limit + 1]);
      return ['bindings -> bindings[1..min(length(bindings), limit)],
        'complete -> result['complete] && length(bindings) <= limit];
    endif
    kind in {'all, 'any} || raise(E_INVARG, "Unknown condition kind");
    let bindings = kind == 'all ? {initial} | {};
    let complete = true;
    for child in (tree['children])
      let next = kind == 'all ? {} | bindings;
      const candidates = kind == 'all ? bindings | {initial};
      for candidate in (candidates)
        let scope = environment;
        scope['binding] = candidate;
        const result = $condition:_evaluate(child, scope, provider, limit, depth + 1);
        complete = complete && result['complete];
        for row in (result['bindings])
          next = $condition:_unique_add(next, row);
          if (length(next) > limit)
            next = next[1..limit];
            complete = false;
          endif
        endfor
      endfor
      bindings = next;
    endfor
    return ['bindings -> bindings, 'complete -> complete];
  endmethod

  method _unique_add owner: HACKER
    "Deduplicate binding maps using exact value equality, including string case.";
    const {rows, candidate} = args;
    for row in (rows)
      equal(row, candidate) && return rows;
    endfor
    return {@rows, candidate};
  endmethod
endobject
