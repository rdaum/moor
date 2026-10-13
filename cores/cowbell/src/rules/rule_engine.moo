object RULE_ENGINE [
  import_export_id -> "rule_engine",
  import_export_hierarchy -> {"rules"}
]
  name: "Rule Engine"
  parent: ROOT
  owner: HACKER
  readable: true

  override description (owner: HACKER, flags: "rc") = "Stateless Datalog-style query engine. Evaluates rules and goals by calling fact predicates on objects.";
  override object_documentation (owner: HACKER, flags: "rc") = {
    "# Rule Engine ($rule_engine)",
    "",
    "## Overview",
    "",
    "The Rule Engine provides Datalog-style rules and queries for MOO objects to be used for constructing logic puzzles, locks, access controls, and object interactions. ",
    "It uses SLD resolution with backtracking to find variable bindings that satisfy logical rules.",
    "",
    "### How It Works",
    "",
    "The engine evaluates rules by:",
    "1. **Unification**: Matching variables to values that satisfy predicates",
    "2. **Resolution**: Proving goals by calling fact predicates on objects",
    "3. **Backtracking**: Trying alternative solutions when predicates fail",
    "",
    "When a rule has variables (like `Key` or `Accessor`), the engine finds bindings that make the rule true.",
    "",
    "### Unification Example",
    "",
    "Given the rule: `Child parent(Parent)? AND Parent parent(Grandparent)?`",
    "",
    "With initial binding `Child` = #123, the engine will:",
    "1. Call `#123:fact_parent(#123, 'Parent)` - predicate returns possible parents",
    "2. Try binding `Parent` to first result (e.g., #456)",
    "3. Call `#456:fact_parent(#456, 'Grandparent)` - predicate returns grandparents",
    "4. Bind `Grandparent` to result (e.g., #789)",
    "5. Return success with bindings: ['Child -> #123, 'Parent -> #456, 'Grandparent -> #789]",
    "6. On backtracking, try alternative parents or grandparents",
    "",
    "This demonstrates transitive resolution - rules can chain relationships:",
    "- `Key is(\"golden key\")?` - finds and binds the key object",
    "- `Child parent(Parent)?` - finds what object is the parent of Child",
    "- `Person1 parent(P1)? AND P1 parent(GP)? AND Person2 parent(P2)? AND P2 parent(GP)?` - finds cousins sharing grandparent GP",
    "",
    "## Rule Syntax",
    "",
    "Rules are written in a builder-friendly syntax:",
    "",
    "```",
    "Variable predicate(args)?",
    "Variable1 predicate1? AND Variable2 predicate2?",
    "Variable1 predicate1? OR Variable2 predicate2?",
    "NOT Variable predicate?",
    "```",
    "",
    "### Variables and Constants",
    "",
    "- **Capitalized names** are variables: `Key`, `Accessor`, `This`",
    "- **Lowercase names** are constants: `player`, `here`",
    "- **Quoted strings** match objects: `\"key\"` matches an object named \"key\"",
    "- **Object literals**: `#123` or `#0000AB-12345678`",
    "- **Numbers**: `42`, `0`",
    "",
    "### Predicates",
    "",
    "Predicates are verbs that return true/false, named with `fact_` prefix:",
    "",
    "- `This is_locked()? ` - calls `this:fact_is_locked()`",
    "- `Key is(#123)?` - calls `#0:fact_is(key, #123)`  ",
    "- `Container contains(Item)?` - calls `#0:fact_contains(container, item)`",
    "",
    "### Operators",
    "",
    "- **AND**: All conditions must be true",
    "- **OR**: At least one condition must be true",
    "- **NOT**: Bounded negation (at most 1 unbound variable allowed)",
    "",
    "## Common Use Cases",
    "",
    "### Container Access Control",
    "",
    "```",
    "@set-rule chest.lock_rule Key is(\"golden key\")?",
    "@set-rule chest.unlock_rule Key is(\"golden key\")?  ",
    "@set-rule chest.take_rule NOT This is_locked()?",
    "@set-rule chest.put_rule NOT This is_locked()?",
    "```",
    "",
    "### Conditional Access",
    "",
    "```",
    "@set-rule door.can_pass Accessor has_permission(\"vip\")?",
    "@set-rule vault.open_rule Accessor is_wizard? OR Key is(\"master key\")?",
    "```",
    "",
    "## Commands",
    "",
    "- `@set-rule <object>.<property> <expression>` - Set a rule",
    "- `@show-rule <object>.<property>` - Display a rule",
    "- `@clear-rule <object>.<property>` - Remove a rule",
    "- `@rules <object>` - List all rules on an object",
    "",
    "## Creating Custom Predicates",
    "",
    "Add `fact_*` verbs to objects:",
    "",
    "```moo",
    "verb fact_has_key (this none this)",
    "  {accessor, required_key} = args;",
    "  return required_key in accessor.contents;",
    "endverb",
    "```",
    "",
    "Then use in rules: `Accessor has_key(\"rusty key\")?`"
  };

  method evaluate owner: ARCH_WIZARD
    "Evaluate a rule to find all satisfying variable bindings.";
    "Returns: {success: bool, bindings: map, alternatives: list of maps, warnings: list}";
    set_task_perms(caller_perms());
    const {rule, ?initial_bindings = []} = args;
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_TYPE, "rule must be flyweight");
    const body = rule.body;
    const warnings = this:_check_negation_warnings(body, initial_bindings);
    "Check if body is OR-structured (nested lists) or AND-structured (flat goals)";
    const is_branches = length(body) > 0 && typeof(body[1]) == TYPE_LIST && length(body[1]) > 0 && typeof(body[1][1]) == TYPE_LIST;
    let result = is_branches ? this:_prove_alternatives(body, initial_bindings) | this:_prove_goals(body, initial_bindings, {});
    result['warnings] = warnings;
    return result;
  endmethod

  method _prove_alternatives owner: ARCH_WIZARD
    "Prove a list of alternative goal branches (OR).";
    set_task_perms(caller_perms());
    const {alternatives, bindings} = args;
    typeof(alternatives) == TYPE_LIST || raise(E_TYPE, "alternatives must be list");
    let all_solutions = {};
    for branch in (alternatives)
      typeof(branch) == TYPE_LIST || raise(E_TYPE, "each branch must be list of goals");
      const result = this:_prove_goals(branch, bindings, {});
      if (result['success])
        all_solutions = {@all_solutions, result['bindings]};
        const branch_alts = result['alternatives];
        if (typeof(branch_alts) == TYPE_LIST)
          for alt in (branch_alts)
            all_solutions = {@all_solutions, alt};
          endfor
        endif
      endif
    endfor
    length(all_solutions) == 0 && return ['success -> false, 'bindings -> [], 'alternatives -> {}];
    return ['success -> true, 'bindings -> all_solutions[1], 'alternatives -> all_solutions[2..$]];
  endmethod

  method _prove_goals owner: ARCH_WIZARD
    "Prove a list of goals with backtracking. Returns all solutions.";
    set_task_perms(caller_perms());
    const {goals, bindings, ?_choice_stack = {}} = args;
    typeof(goals) == TYPE_LIST || raise(E_TYPE, "goals must be list");
    typeof(bindings) == TYPE_MAP || raise(E_TYPE, "bindings must be map");
    length(goals) == 0 && return ['success -> true, 'bindings -> bindings, 'alternatives -> {}];
    const first_goal = goals[1];
    const rest_goals = listdelete(goals, 1);
    "Handle negation as failure";
    if (length(first_goal) > 0 && first_goal[1] == 'not)
      const inner_goals = listdelete(first_goal, 1);
      const inner_result = this:_prove_goals(inner_goals, bindings);
      inner_result['success] && return ['success -> false, 'bindings -> [], 'alternatives -> {}];
      return this:_prove_goals(rest_goals, bindings);
    endif
    "Get all solutions for the first goal, then prove rest with each";
    let all_results = {};
    for solution_bindings in (this:_solve_goal(first_goal, bindings))
      const rest_result = this:_prove_goals(rest_goals, solution_bindings);
      if (rest_result['success])
        all_results = {@all_results, rest_result['bindings], @rest_result['alternatives]};
      endif
    endfor
    length(all_results) == 0 && return ['success -> false, 'bindings -> [], 'alternatives -> {}];
    return ['success -> true, 'bindings -> all_results[1], 'alternatives -> all_results[2..$]];
  endmethod

  method _solve_goal owner: ARCH_WIZARD
    "Solve a single goal by calling fact predicates. Returns list of bindings.";
    set_task_perms(caller_perms());
    const {goal, bindings} = args;
    typeof(goal) == TYPE_LIST || raise(E_TYPE, "goal must be list");
    length(goal) >= 1 || raise(E_INVARG, "goal must have predicate name");
    const predicate_name = goal[1];
    const goal_args = goal[2..$];
    typeof(predicate_name) == TYPE_STR || typeof(predicate_name) == TYPE_SYM || raise(E_TYPE, "predicate name must be string or symbol");
    const substituted_args = this:_substitute_args(goal_args, bindings);
    length(substituted_args) == 0 && raise(E_INVARG, "goal needs at least one argument (the object)");
    const target_obj = substituted_args[1];
    typeof(target_obj) == TYPE_OBJ || raise(E_TYPE, "first goal argument must be object");
    let fact_results = target_obj:("fact_" + tostr(predicate_name))(@substituted_args);
    "Check for failure (false, 0, empty string, empty list - but NOT valid objects)";
    if (fact_results == false || fact_results == 0 || fact_results == "" || fact_results == {})
      return {};
    endif
    typeof(fact_results) != TYPE_LIST && (fact_results = {fact_results});
    "Unify each result with the original goal to get bindings";
    let unified_solutions = {};
    for result in (fact_results)
      const new_bindings = this:_unify_goal(goal, result, bindings);
      new_bindings != false && (unified_solutions = {@unified_solutions, new_bindings});
    endfor
    return unified_solutions;
  endmethod

  method _substitute_args owner: HACKER
    "Replace variables in args with their bindings.";
    {args, bindings} = args;
    substituted = term_substitute(args, bindings, ['unbound -> 'leave]);
    return this:_predicate_call_value(substituted, bindings);
  endmethod

  method _substitute_value owner: HACKER
    "Substitute a single value, resolving variables.";
    {value, bindings} = args;
    return this:_predicate_call_value(term_substitute(value, bindings, ['unbound -> 'leave]), bindings);
  endmethod

  method _predicate_call_value owner: HACKER
    "Convert unbound variables and bound legacy symbols for fact predicate calls.";
    {value, bindings} = args;
    this:_is_var(value) && return this:_var_name(value);
    typeof(value) == TYPE_SYM && maphaskey(bindings, value) && return bindings[value];
    if (typeof(value) == TYPE_LIST)
      result = {};
      for item in (value)
        result = {@result, this:_predicate_call_value(item, bindings)};
      endfor
      return result;
    endif
    return value;
  endmethod

  method _unify_goal owner: HACKER
    "Unify a goal with a result, returning new bindings or false.";
    {goal, result, bindings} = args;
    goal_args = goal[2..$];
    "Find first unbound variable and bind it to result";
    for i in [1..length(goal_args)]
      arg = goal_args[i];
      if (this:_is_var(arg) && !maphaskey(bindings, this:_var_name(arg)))
        return term_unify(arg, result, bindings);
      endif
    endfor
    "No unbound variables - ground goal. Only explicit false means failure.";
    return result == false ? false | bindings;
  endmethod

  method _is_var owner: HACKER
    "Return true when value is a normalized logic variable marker.";
    {value} = args;
    return typeof(value) == TYPE_LIST && length(value) == 2 && value[1] == 'var && typeof(value[2]) == TYPE_SYM;
  endmethod

  method _var_name owner: HACKER
    "Return the symbol identifier from a normalized logic variable marker.";
    {value} = args;
    this:_is_var(value) || raise(E_TYPE, "value is not a variable marker");
    return value[2];
  endmethod

  method parse_expression owner: HACKER
    "Parse a builder-friendly expression into a rule flyweight.";
    "Expression syntax: 'Object predicate? AND Object predicate2? OR NOT Object predicate3?'";
    "Capitalized words are variables, lowercase are constants. ? marks a predicate.";
    "Quoted strings like \"key\" are matched to objects from match_perspective.";
    {expression_string, ?rule_name = 'parsed_rule, ?match_perspective = player} = args;
    typeof(expression_string) == TYPE_STR || raise(E_TYPE, "expression must be string");
    "Tokenize the expression";
    tokens = this:_tokenize(expression_string);
    "Parse tokens into goal list";
    goals = this:_parse_goals(tokens, match_perspective);
    "Extract variables from goals";
    variables = this:_extract_variables_from_goals(goals);
    "Return rule flyweight";
    return <$rule, .name = rule_name, .head = rule_name, .body = goals, .variables = variables>;
  endmethod

  method validate_rule owner: HACKER
    "Validate a rule for bounded negation violations without evaluating it.";
    "Returns: {valid, warnings} where warnings is list of error/warning strings";
    {rule} = args;
    warnings = {};
    goals = rule.body;
    all_vars = rule.variables;
    "Check each branch for bounded negation violations";
    "A goal is {predicate, arg1, ...} where predicate is SYM";
    "Single branch: goals = {goal1, goal2, ...}";
    "OR expression: goals = {branch1, branch2, ...} where branch is {goal1, goal2, ...}";
    "So: if goals[1] is LIST and goals[1][1] is LIST, then it's OR (branches of goals)";
    "    if goals[1] is LIST and goals[1][1] is SYM, then single branch (list of goals)";
    if (length(goals) > 0 && typeof(goals[1]) == TYPE_LIST)
      if (length(goals[1]) > 0 && typeof(goals[1][1]) == TYPE_LIST)
        "OR expression - goals is list of branches";
        for branch in (goals)
          warnings = {@warnings, @this:_check_branch_negation(branch, all_vars)};
        endfor
      else
        "Single branch - goals is a list of goals";
        warnings = {@warnings, @this:_check_branch_negation(goals, all_vars)};
      endif
    endif
    "Check if any errors (vs warnings)";
    has_error = false;
    for warning in (warnings)
      if (index(warning, "ERROR:") > 0)
        has_error = true;
      endif
    endfor
    return ['valid -> !has_error, 'warnings -> warnings];
  endmethod

  method _check_branch_negation owner: HACKER
    "Check a single branch for bounded negation violations.";
    {branch, all_vars} = args;
    warnings = {};
    bound_vars = [];
    for goal in (branch)
      "Check if this is a negated goal";
      if (typeof(goal) == TYPE_LIST && length(goal) > 0 && goal[1] == 'not)
        "Negated goal - check bounded negation";
        inner_goal = goal[2];
        "Find unbound variables in this negated goal";
        unbound_vars = [];
        for arg in (inner_goal[2..$])
          if (this:_is_var(arg) && !maphaskey(bound_vars, this:_var_name(arg)))
            unbound_vars[this:_var_name(arg)] = true;
          endif
        endfor
        "Check if we have 2+ unbound variables (not allowed)";
        if (length(unbound_vars) > 1)
          warnings = {@warnings, "ERROR: Negation has " + tostr(length(unbound_vars)) + " unbound variables: " + toliteral(mapkeys(unbound_vars)) + " in goal " + toliteral(inner_goal) + " - bounded negation allows at most 1 unbound variable"};
        endif
      else
        "Positive goal - track which variables get bound";
        for arg in (goal[2..$])
          if (this:_is_var(arg) && this:_var_name(arg) in all_vars)
            bound_vars[this:_var_name(arg)] = true;
          endif
        endfor
      endif
    endfor
    return warnings;
  endmethod

  method _tokenize owner: HACKER
    "Tokenize an expression string into a list of tokens.";
    "Returns: list of strings, each token is a word, operator, or complex term";
    "Complex terms like 'predicate(arg)?' are kept as single tokens";
    {expression_string} = args;
    tokens = {};
    current_token = "";
    i = 1;
    while (i <= length(expression_string))
      char = expression_string[i];
      "Check for quoted strings";
      if (char == "\"")
        "Scan for closing quote";
        if (length(current_token) > 0)
          tokens = {@tokens, current_token};
          current_token = "";
        endif
        quoted_string = "\"";
        i = i + 1;
        while (i <= length(expression_string) && expression_string[i] != "\"")
          quoted_string = quoted_string + expression_string[i];
          i = i + 1;
        endwhile
        if (i > length(expression_string))
          raise(E_INVARG, "Unterminated quoted string");
        endif
        quoted_string = quoted_string + "\"";
        tokens = {@tokens, quoted_string};
        i = i + 1;
        continue;
      endif
      "Check for whitespace";
      if (char == " " || char == "\t" || char == "\n")
        if (length(current_token) > 0)
          tokens = {@tokens, current_token};
          current_token = "";
        endif
        i = i + 1;
        continue;
      endif
      "Check for ( - start of predicate arguments or expression grouping";
      if (char == "(")
        "Check if we have a predicate name (in current_token or last token)";
        predicate_name = "";
        if (length(current_token) > 0)
          predicate_name = current_token;
        elseif (length(tokens) > 0)
          "Check if last token could be a predicate name";
          last_token = tokens[length(tokens)];
          "Predicate names are lowercase identifiers";
          if (last_token[1] >= "a" && last_token[1] <= "z")
            predicate_name = last_token;
            "Remove it from tokens, we'll rebuild with args";
            tokens = tokens[1..length(tokens) - 1];
          endif
        endif
        if (predicate_name != "")
          "Look ahead to see if this is a predicate with arguments";
          paren_count = 1;
          j = i + 1;
          while (j <= length(expression_string) && paren_count > 0)
            if (expression_string[j] == "(")
              paren_count = paren_count + 1;
            elseif (expression_string[j] == ")")
              paren_count = paren_count - 1;
            endif
            j = j + 1;
          endwhile
          "j now points after the closing ), check for ?";
          if (j <= length(expression_string) && expression_string[j] == "?")
            "This is a predicate with args like 'parent(X)?'";
            "Build the full predicate token";
            pred_token = predicate_name;
            while (i <= j)
              pred_token = pred_token + expression_string[i];
              i = i + 1;
            endwhile
            tokens = {@tokens, pred_token};
            current_token = "";
            continue;
          else
            "Not a predicate - restore last token if we removed it";
            if (predicate_name != current_token)
              tokens = {@tokens, predicate_name};
            endif
            "Regular parenthesis for grouping";
            if (length(current_token) > 0)
              tokens = {@tokens, current_token};
              current_token = "";
            endif
            tokens = {@tokens, char};
            i = i + 1;
            continue;
          endif
        else
          "Standalone ( for grouping";
          tokens = {@tokens, char};
          i = i + 1;
          continue;
        endif
      endif
      "Check for )";
      if (char == ")")
        if (length(current_token) > 0)
          tokens = {@tokens, current_token};
          current_token = "";
        endif
        tokens = {@tokens, char};
        i = i + 1;
        continue;
      endif
      "Accumulate into current token";
      current_token = current_token + char;
      i = i + 1;
    endwhile
    "Don't forget the last token";
    if (length(current_token) > 0)
      tokens = {@tokens, current_token};
    endif
    return tokens;
  endmethod

  method _parse_goals owner: HACKER
    "Parse a token list into a goal structure.";
    "Handles AND, OR, NOT operators and parentheses.";
    "Returns either a flat goal list (no OR) or a list of branches (OR present)";
    {tokens, match_perspective} = args;
    "Parse starting from position 1, return (branches, next_position)";
    result = this:_parse_or_expression(tokens, 1, match_perspective);
    if (result[2] <= length(tokens))
      raise(E_INVARG, "Unexpected token at position " + tostr(result[2]) + ": " + tokens[result[2]]);
    endif
    branches = result[1];
    "If only one branch, unwrap it to maintain backward compatibility";
    if (length(branches) == 1)
      return branches[1];
    else
      "Multiple branches (OR expressions) - return as list of branches";
      return branches;
    endif
  endmethod

  method _parse_or_expression owner: HACKER
    "Parse OR expressions: term OR term OR term";
    "Returns: {branches_list, next_position}";
    "Each branch is a list of goals (AND'd together)";
    {tokens, pos, match_perspective} = args;
    "Parse first AND expression";
    result = this:_parse_and_expression(tokens, pos, match_perspective);
    branch = result[1];
    pos = result[2];
    branches = {branch};
    "Parse remaining OR terms";
    while (pos <= length(tokens) && tokens[pos]:lowercase() == "or")
      pos = pos + 1;
      "Parse next AND expression";
      result = this:_parse_and_expression(tokens, pos, match_perspective);
      or_branch = result[1];
      pos = result[2];
      branches = {@branches, or_branch};
    endwhile
    return {branches, pos};
  endmethod

  method _parse_and_expression owner: HACKER
    "Parse AND expressions: term AND term AND term";
    "Returns: {goals_list, next_position}";
    {tokens, pos, match_perspective} = args;
    "Parse first term";
    result = this:_parse_term(tokens, pos, match_perspective);
    goals = result[1];
    pos = result[2];
    "Parse remaining AND terms";
    while (pos <= length(tokens) && tokens[pos]:lowercase() == "and")
      pos = pos + 1;
      "Parse next term";
      result = this:_parse_term(tokens, pos, match_perspective);
      term_goals = result[1];
      pos = result[2];
      goals = {@goals, @term_goals};
    endwhile
    return {goals, pos};
  endmethod

  method _parse_term owner: HACKER
    "Parse a single term: either 'NOT term', '(expression)', or 'object predicate(args)?'";
    "Returns: {goals_list, next_position}";
    {tokens, pos, match_perspective} = args;
    pos <= length(tokens) || raise(E_INVARG, "Unexpected end of expression");
    current = tokens[pos];
    "Handle NOT - negation as failure";
    if (current:lowercase() == "not")
      pos = pos + 1;
      result = this:_parse_term(tokens, pos, match_perspective);
      inner_goals = result[1];
      pos = result[2];
      "Wrap goals in 'not' marker: {not, goal1, goal2, ...}";
      not_goal = {'not};
      for goal in (inner_goals)
        not_goal = {@not_goal, goal};
      endfor
      return {{not_goal}, pos};
    endif
    "Handle parentheses";
    if (current == "(")
      pos = pos + 1;
      result = this:_parse_or_expression(tokens, pos, match_perspective);
      goals = result[1];
      pos = result[2];
      pos <= length(tokens) || raise(E_INVARG, "Missing closing parenthesis");
      tokens[pos] == ")" || raise(E_INVARG, "Expected closing parenthesis");
      pos = pos + 1;
      return {goals, pos};
    endif
    "Handle object predicate(args)? syntax";
    object_name = current;
    pos = pos + 1;
    pos <= length(tokens) || raise(E_INVARG, "Expected predicate after object");
    predicate_spec = tokens[pos];
    pos = pos + 1;
    "Check if predicate has arguments: predicate(arg1, arg2)?";
    pred_args = {};
    if (index(predicate_spec, "(") > 0)
      "Parse predicate with arguments";
      result = this:_parse_predicate_with_args(predicate_spec);
      predicate_name = result[1];
      pred_args = result[2];
    else
      "Simple predicate without arguments";
      "Check that predicate ends with ?";
      predicate_spec[length(predicate_spec)] == "?" || raise(E_INVARG, "Predicate must end with ?");
      "Strip the ?";
      predicate_name = predicate_spec[1..length(predicate_spec) - 1];
    endif
    "Convert object name to symbol or object reference";
    object_arg = this:_resolve_name(object_name, match_perspective);
    "Build goal: {predicate_name, object_arg, ...pred_args}";
    goal = {tosym(predicate_name:lowercase()), object_arg};
    for arg in (pred_args)
      arg_ref = this:_resolve_name(arg, match_perspective);
      goal = {@goal, arg_ref};
    endfor
    return {{goal}, pos};
  endmethod

  method _parse_predicate_with_args owner: HACKER
    "Parse a predicate with arguments: 'predicate(arg1, arg2)?'";
    "Returns: {predicate_name, args_list}";
    {predicate_spec} = args;
    "Find opening paren";
    paren_pos = index(predicate_spec, "(");
    paren_pos > 0 || raise(E_INVARG, "Expected ( in predicate");
    predicate_name = predicate_spec[1..paren_pos - 1];
    "Find closing paren and ?";
    close_pos = index(predicate_spec, ")");
    close_pos > paren_pos || raise(E_INVARG, "Expected ) in predicate");
    close_pos < length(predicate_spec) || raise(E_INVARG, "Expected ? after )");
    predicate_spec[length(predicate_spec)] == "?" || raise(E_INVARG, "Predicate must end with ?");
    "Extract arguments between ( and )";
    args_str = predicate_spec[paren_pos + 1..close_pos - 1];
    "Split by comma";
    args = {};
    if (length(args_str) > 0)
      "Simple comma splitting (doesn't handle nested parens, but good enough for now)";
      current_arg = "";
      for i in [1..length(args_str)]
        char = args_str[i];
        if (char == ",")
          arg = current_arg:trim();
          length(arg) > 0 || raise(E_INVARG, "Empty argument in predicate");
          args = {@args, arg};
          current_arg = "";
        else
          current_arg = current_arg + char;
        endif
      endfor
      "Don't forget the last argument";
      if (length(current_arg) > 0)
        arg = current_arg:trim();
        args = {@args, arg};
      endif
    endif
    return {predicate_name, args};
  endmethod

  method _resolve_name owner: HACKER
    "Resolve a name to variable marker, object, or constant.";
    {name, match_perspective} = args;
    "Quoted string - match to object";
    if (length(name) >= 2 && name[1] == "\"" && name[$] == "\"")
      obj_name = name[2..$ - 1];
      matched_obj = $match:match_object(obj_name, match_perspective);
      valid(matched_obj) || raise(E_INVARG, "Could not match object: \"" + obj_name + "\"");
      return matched_obj;
    endif
    "Object literal";
    if (length(name) > 0 && name[1] == "#")
      obj_val = toobj(name);
      obj_val != #0 || name == "#0" && return obj_val;
    endif
    "Integer literal";
    num_val = toint(name);
    num_val != 0 || name == "0" && return num_val;
    "Uppercase = variable";
    first_char = name[1];
    index("ABCDEFGHIJKLMNOPQRSTUVWXYZ", first_char, 1) && return {'var, tosym(name)};
    "Known constants";
    constants = ["player" -> 'player, "this" -> 'this, "sender" -> 'sender, "location" -> 'location, "sysobj" -> 'sysobj];
    lower_name = name:lowercase();
    maphaskey(constants, lower_name) && return constants[lower_name];
    raise(E_INVARG, "Unknown object constant: " + name);
  endmethod

  method _extract_variables_from_goals owner: HACKER
    "Extract all variables from a goal list.";
    {goals} = args;
    variables = {};
    variables = this:_collect_variables(goals, variables);
    return variables;
  endmethod

  method _collect_variables owner: HACKER
    "Append normalized variable names found in value to variables.";
    {value, variables} = args;
    if (this:_is_var(value))
      name = this:_var_name(value);
      !(name in variables) && (variables = {@variables, name});
      return variables;
    endif
    if (typeof(value) == TYPE_LIST)
      for item in (value)
        variables = this:_collect_variables(item, variables);
      endfor
    endif
    return variables;
  endmethod

  method _check_negation_warnings owner: HACKER
    "Check for problematic negation patterns and return warnings/errors.";
    "Bounded negation: 0-1 unbound variables OK, 2+ is an error.";
    "Returns: list of warning/error strings";
    {goals, current_bindings} = args;
    warnings = {};
    for goal in (goals)
      if (typeof(goal) == TYPE_LIST && length(goal) > 0 && goal[1] == 'not)
        "This is a negated goal";
        inner_goals = listdelete(goal, 1);
        "Count unbound variables across all inner goals";
        unbound_vars = {};
        for inner_goal in (inner_goals)
          if (typeof(inner_goal) == TYPE_LIST && length(inner_goal) >= 2)
            for i in [2..length(inner_goal)]
              arg = inner_goal[i];
              if (this:_is_var(arg) && !maphaskey(current_bindings, this:_var_name(arg)))
                "Collect unbound variables (deduplicate)";
                name = this:_var_name(arg);
                if (!(name in unbound_vars))
                  unbound_vars = {@unbound_vars, name};
                endif
              endif
            endfor
          endif
        endfor
        "Check if we have 2+ unbound variables (not allowed)";
        if (length(unbound_vars) > 1)
          warnings = {@warnings, "ERROR: Negation has " + tostr(length(unbound_vars)) + " unbound variables: " + tostr(unbound_vars) + " in goal " + tostr(goal) + " - bounded negation allows at most 1 unbound variable"};
        elseif (length(unbound_vars) == 1)
          "Single unbound variable is OK (bounded negation)";
        endif
      endif
    endfor
    return warnings;
  endmethod

  method decompile_rule owner: HACKER
    "Convert a rule flyweight back to DSL expression string.";
    {rule} = args;
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_TYPE, "rule must be flyweight");
    body = rule.body;
    "Check if body is OR-structured (nested lists) or AND-structured (flat goals)";
    is_or = length(body) > 0 && typeof(body[1]) == TYPE_LIST && length(body[1]) > 0 && typeof(body[1][1]) == TYPE_LIST;
    is_or || return this:_decompile_goals(body);
    return { this:_decompile_goals(branch) for branch in (body) }:join(" OR ");
  endmethod

  method _decompile_goals owner: HACKER
    "Decompile a list of goals into DSL syntax.";
    {goals} = args;
    return { this:_decompile_goal(g) for g in (goals) }:join(" AND ");
  endmethod

  method _decompile_goal owner: HACKER
    "Decompile a single goal into DSL syntax.";
    {goal} = args;
    typeof(goal) == TYPE_LIST || raise(E_TYPE, "goal must be list");
    length(goal) >= 1 || raise(E_INVARG, "goal must have predicate");
    goal[1] == 'not && return "NOT " + this:_decompile_goals(listdelete(goal, 1));
    predicate = goal[1];
    goal_args = goal[2..$];
    length(goal_args) == 0 && raise(E_INVARG, "goal must have at least object argument");
    obj_str = this:_decompile_value(goal_args[1]);
    remaining_args = goal_args[2..$];
    predicate_str = tostr(predicate);
    length(remaining_args) > 0 && (predicate_str = predicate_str + "(" + { this:_decompile_value(a) for a in (remaining_args) }:join(", ") + ")");
    return obj_str + " " + predicate_str + "?";
  endmethod

  method _decompile_value owner: HACKER
    "Convert a value back to DSL representation.";
    {value} = args;
    this:_is_var(value) && return tostr(this:_var_name(value));
    return tostr(value);
  endmethod

  method test_simple_goal owner: ARCH_WIZARD
    "Test evaluating a simple goal.";
    test_obj = #-1;
    try
      "Create a test object with a fact predicate";
      test_obj = $root:create(false);
      test_obj.name = "Test Object";
      "Define a simple fact: fact_true() returns true";
      "info: {owner, perms, names}";
      "args: {dobj, prep, iobj}";
      add_verb(test_obj, {#2, "rxd", "fact_true"}, {"this", "none", "none"});
      set_verb_code(test_obj, "fact_true", {"return true;"});
      "Evaluate a goal";
      "Note: predicate name is 'true', not 'fact_true' - the fact_ prefix is added by _solve_goal";
      empty_bindings = [];
      goal = {'true, test_obj};
      result = this:_solve_goal(goal, empty_bindings);
      typeof(result) == TYPE_LIST || raise(E_ASSERT, "Result should be list");
      length(result) > 0 || raise(E_ASSERT, "Should have at least one solution");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_conjunction owner: ARCH_WIZARD
    "Test proving multiple goals (conjunction).";
    "Two goals: reputation >= 5 AND reputation >= 3 (both should succeed)";
    test_guild = #64;
    goals = {{'reputation, test_guild, 5}, {'reputation, test_guild, 3}};
    empty_bindings = [];
    result = this:_prove_goals(goals, empty_bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    result['success] || raise(E_ASSERT, "Both goals should succeed");
    return true;
  endmethod

  method test_failed_goal owner: ARCH_WIZARD
    "Test a goal that should fail.";
    "Reputation >= 10 should fail (#64 has reputation 8)";
    test_guild = #64;
    goal = {'reputation, test_guild, 10};
    empty_bindings = [];
    result = this:_solve_goal(goal, empty_bindings);
    typeof(result) == TYPE_LIST || raise(E_ASSERT, "Result should be list");
    length(result) == 0 || raise(E_ASSERT, "Should have no solutions");
    return true;
  endmethod

  method test_variable_binding owner: ARCH_WIZARD
    "Test variable unification in goals.";
    test_obj = #-1;
    try
      "Query: what X satisfies parent(test_obj, X)?";
      test_obj = #64:create(false);
      "Set up parent relationship";
      test_obj.father = $root;
      "Create goal with variable X in second position";
      goal = {'parent, test_obj, {'var, 'X}};
      empty_bindings = [];
      result = this:_solve_goal(goal, empty_bindings);
      typeof(result) == TYPE_LIST || raise(E_ASSERT, "Result should be list");
      length(result) > 0 || raise(E_ASSERT, "Should have at least one solution");
      "First solution should bind 'X to $root";
      first_solution = result[1];
      typeof(first_solution) == TYPE_MAP || raise(E_ASSERT, "Solution should be map");
      first_solution['X] == $root || raise(E_ASSERT, "'X should be bound to $root");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_multiple_solutions owner: ARCH_WIZARD
    "Test goals that have multiple solutions via alternatives.";
    test_obj = #-1;
    try
      "Create a test object with both father and mother";
      test_obj = #64:create(false);
      test_obj.father = $root;
      test_obj.mother = $arch_wizard;
      "Query: what X satisfies parent(test_obj, X)?";
      goal = {'parent, test_obj, {'var, 'X}};
      empty_bindings = [];
      result = this:_prove_goals({goal}, empty_bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Goal should succeed");
      "Check that we have alternatives (multiple solutions)";
      alternatives = result['alternatives];
      typeof(alternatives) == TYPE_LIST || raise(E_ASSERT, "Alternatives should be list");
      length(alternatives) > 0 || raise(E_ASSERT, "Should have alternative solutions");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_transitive_uncle owner: ARCH_WIZARD
    "Test transitive relationship: uncle = parent's parent.";
    test_obj = #-1;
    mother_obj = #-1;
    try
      "Set up family: test_obj has mother, mother has father";
      test_obj = #64:create(false);
      "Create mother object (inherit from RULE_TEST to get father/mother properties)";
      mother_obj = #64:create(false);
      test_obj.mother = mother_obj;
      "Set mother's father (test_obj's grandfather)";
      mother_obj.father = $arch_wizard;
      "Query: what X satisfies parent(test_obj, Y) AND parent(Y, X)?";
      "This should find test_obj's grandparents";
      goals = {{'parent, test_obj, {'var, 'Y}}, {'parent, {'var, 'Y}, {'var, 'X}}};
      empty_bindings = [];
      result = this:_prove_goals(goals, empty_bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should find grandparent");
      "Verify that X is bound to the grandfather";
      first_binding = result['bindings];
      typeof(first_binding) == TYPE_MAP || raise(E_ASSERT, "Binding should be map");
      first_binding['X] == $arch_wizard || raise(E_ASSERT, "X should be $arch_wizard (grandfather)");
      first_binding['Y] == mother_obj || raise(E_ASSERT, "Y should be mother_obj");
    finally
      valid(mother_obj) && mother_obj:destroy();
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_cousin_relationship owner: ARCH_WIZARD
    "Test cousin relationship: verify cousin shares grandparent.";
    test_obj = #-1;
    mother_obj = #-1;
    sibling_obj = #-1;
    cousin_obj = #-1;
    try
      "Build family tree:";
      "  $arch_wizard (grandparent)";
      "    |";
      "    +-- mother_obj (parent)";
      "    |      |";
      "    |      +-- test_obj (ego)";
      "    |";
      "    +-- sibling_obj (parent's sibling)";
      "           |";
      "           +-- cousin_obj (cousin)";
      test_obj = #64:create(false);
      test_obj.father = #0;
      test_obj.mother = #0;
      "Create mother";
      mother_obj = #64:create(false);
      test_obj.mother = mother_obj;
      mother_obj.father = $arch_wizard;
      "Create sibling of mother";
      sibling_obj = #64:create(false);
      sibling_obj.father = $arch_wizard;
      "Create cousin (sibling's child)";
      cousin_obj = #64:create(false);
      cousin_obj.father = sibling_obj;
      "Query: cousin_obj and test_obj share a grandparent";
      "Find: does cousin_obj's grandparent equal test_obj's grandparent?";
      goals_test_obj = {{'parent, test_obj, {'var, 'P1}}, {'parent, {'var, 'P1}, {'var, 'TestGrandparent}}};
      result_test = this:_prove_goals(goals_test_obj, []);
      typeof(result_test) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result_test['success] || raise(E_ASSERT, "Should find test_obj's grandparent");
      test_grandparent = result_test['bindings]['TestGrandparent];
      "Now check cousin_obj's grandparent";
      goals_cousin = {{'parent, cousin_obj, {'var, 'P2}}, {'parent, {'var, 'P2}, {'var, 'CousinGrandparent}}};
      result_cousin = this:_prove_goals(goals_cousin, []);
      typeof(result_cousin) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result_cousin['success] || raise(E_ASSERT, "Should find cousin's grandparent");
      cousin_grandparent = result_cousin['bindings]['CousinGrandparent];
      "Verify they share the same grandparent";
      test_grandparent == cousin_grandparent || raise(E_ASSERT, "Cousin and test_obj should share grandparent");
    finally
      valid(cousin_obj) && cousin_obj:destroy();
      valid(sibling_obj) && sibling_obj:destroy();
      valid(mother_obj) && mother_obj:destroy();
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_parse_simple_expression owner: ARCH_WIZARD
    "Test parsing a simple expression into a rule.";
    "Parse: 'player has_key?'";
    expression = "player has_key?";
    rule = this:parse_expression(expression, 'simple_parse_test);
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Should return flyweight");
    rule.head == 'simple_parse_test || raise(E_ASSERT, "Rule head should match");
    typeof(rule.body) == TYPE_LIST || raise(E_ASSERT, "Rule body should be list");
    length(rule.body) == 1 || raise(E_ASSERT, "Should have 1 goal");
    "Check the goal structure";
    goal = rule.body[1];
    typeof(goal) == TYPE_LIST || raise(E_ASSERT, "Goal should be list");
    length(goal) == 2 || raise(E_ASSERT, "Goal should have 2 elements");
    goal[1] == 'has_key || raise(E_ASSERT, "Predicate should be has_key");
    return true;
  endmethod

  method test_parse_variable_expression owner: ARCH_WIZARD
    "Test parsing expression with variables.";
    "Parse: 'Player has_key? AND Player is_trusted?'";
    expression = "Player has_key? AND Player is_trusted?";
    rule = this:parse_expression(expression, 'variable_test);
    typeof(rule.body) == TYPE_LIST || raise(E_ASSERT, "Body should be list");
    length(rule.body) == 2 || raise(E_ASSERT, "Should have 2 goals");
    "Check variables were extracted";
    length(rule.variables) > 0 || raise(E_ASSERT, "Should have variables");
    'Player in rule.variables || raise(E_ASSERT, "Should have Player variable");
    "Check first goal";
    goal1 = rule.body[1];
    goal1[1] == 'has_key || raise(E_ASSERT, "First predicate should be has_key");
    goal1[2] == {'var, 'Player} || raise(E_ASSERT, "First goal arg should be Player variable");
    "Check second goal";
    goal2 = rule.body[2];
    goal2[1] == 'is_trusted || raise(E_ASSERT, "Second predicate should be is_trusted");
    goal2[2] == {'var, 'Player} || raise(E_ASSERT, "Second goal arg should be Player variable");
    return true;
  endmethod

  method test_parse_mixed_expression owner: ARCH_WIZARD
    "Test parsing expression with both constants and variables.";
    "Parse: 'Player has_key? AND location is_open?'";
    expression = "Player has_key? AND location is_open?";
    rule = this:parse_expression(expression, 'mixed_test);
    length(rule.body) == 2 || raise(E_ASSERT, "Should have 2 goals");
    goal1 = rule.body[1];
    goal1[1] == 'has_key || raise(E_ASSERT, "First predicate should be has_key");
    goal1[2] == {'var, 'Player} || raise(E_ASSERT, "First arg should be Player variable");
    goal2 = rule.body[2];
    goal2[1] == 'is_open || raise(E_ASSERT, "Second predicate should be is_open");
    goal2[2] == 'location || raise(E_ASSERT, "Second arg should be location constant");
    return true;
  endmethod

  method test_parse_and_evaluate_family owner: ARCH_WIZARD
    "Test parsing a family relationship expression and evaluating it.";
    test_obj = #-1;
    mother_obj = #-1;
    try
      "Parse: 'Child parent(Grandparent)? AND Grandparent parent(GreatGrandparent)?'";
      expression = "Child parent(Grandparent)? AND Grandparent parent(GreatGrandparent)?";
      rule = this:parse_expression(expression, 'find_ancestors);
      "Set up family: test_obj has mother, mother has father";
      test_obj = #64:create(false);
      test_obj.father = #0;
      test_obj.mother = #0;
      mother_obj = #64:create(false);
      test_obj.mother = mother_obj;
      mother_obj.father = $arch_wizard;
      "Now evaluate the rule with bindings: Child=test_obj";
      "This should find: Grandparent=mother_obj, GreatGrandparent=$arch_wizard";
      bindings = ['Child -> test_obj];
      result = this:evaluate(rule, bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should find ancestors");
      "Check bindings";
      result_bindings = result['bindings];
      result_bindings['Grandparent] == mother_obj || raise(E_ASSERT, "Grandparent should be mother_obj");
      result_bindings['GreatGrandparent] == $arch_wizard || raise(E_ASSERT, "GreatGrandparent should be $arch_wizard");
    finally
      valid(mother_obj) && mother_obj:destroy();
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_parse_or_expression owner: ARCH_WIZARD
    "Test parsing OR expressions from strings.";
    "Parse: 'Player has_key? OR Player has_lockpick?'";
    expression = "Player has_key? OR Player has_lockpick?";
    rule = this:parse_expression(expression, 'key_or_pick);
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Should return flyweight");
    typeof(rule.body) == TYPE_LIST || raise(E_ASSERT, "Body should be list");
    length(rule.body) == 2 || raise(E_ASSERT, "Should have 2 branches");
    "Check first branch";
    branch1 = rule.body[1];
    typeof(branch1) == TYPE_LIST || raise(E_ASSERT, "Branch should be list");
    length(branch1) == 1 || raise(E_ASSERT, "First branch should have 1 goal");
    branch1[1][1] == 'has_key || raise(E_ASSERT, "First goal should be has_key");
    "Check second branch";
    branch2 = rule.body[2];
    typeof(branch2) == TYPE_LIST || raise(E_ASSERT, "Branch should be list");
    length(branch2) == 1 || raise(E_ASSERT, "Second branch should have 1 goal");
    branch2[1][1] == 'has_lockpick || raise(E_ASSERT, "Second goal should be has_lockpick");
    return true;
  endmethod

  method test_parse_and_evaluate_or owner: ARCH_WIZARD
    "Test parsing and evaluating OR expressions.";
    test_obj = #-1;
    try
      "Parse: 'Player has_key? OR Player has_lockpick?'";
      expression = "Player has_key? OR Player has_lockpick?";
      rule = this:parse_expression(expression, 'key_or_pick);
      test_obj = #64:create(false);
      "Define a false key predicate and a true lockpick predicate.";
      add_verb(test_obj, {#2, "rxd", "fact_has_key"}, {"this", "none", "none"});
      set_verb_code(test_obj, "fact_has_key", {"return false;"});
      add_verb(test_obj, {#2, "rxd", "fact_has_lockpick"}, {"this", "none", "none"});
      set_verb_code(test_obj, "fact_has_lockpick", {"return true;"});
      "Evaluate rule with Player=test_obj";
      bindings = ['Player -> test_obj];
      result = this:evaluate(rule, bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should succeed (lockpick branch works)");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_or_alternatives owner: ARCH_WIZARD
    "Test evaluating OR expressions with alternatives.";
    test_obj = #-1;
    try
      "Query: (Player has_key?) OR (Player has_lockpick?)";
      test_obj = #64:create(false);
      "Branch 1: Player has_key (will fail)";
      branch1 = {{'has_key, test_obj}};
      "Branch 2: Player has_lockpick (will succeed)";
      branch2 = {{'has_lockpick, test_obj}};
      "Define a false key predicate and a true lockpick predicate.";
      add_verb(test_obj, {#2, "rxd", "fact_has_key"}, {"this", "none", "none"});
      set_verb_code(test_obj, "fact_has_key", {"return false;"});
      add_verb(test_obj, {#2, "rxd", "fact_has_lockpick"}, {"this", "none", "none"});
      set_verb_code(test_obj, "fact_has_lockpick", {"return true;"});
      "Prove alternatives: branch1 OR branch2";
      alternatives = {branch1, branch2};
      result = this:_prove_alternatives(alternatives, []);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should succeed (second branch succeeds)");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_parse_and_evaluate_cousin owner: ARCH_WIZARD
    "Test parsing cousin relationship: find shared grandparents.";
    person1_obj = #-1;
    mother1_obj = #-1;
    mother2_obj = #-1;
    person2_obj = #-1;
    try
      "Parse: 'Person1 parent(Parent1)? AND Parent1 parent(Grandparent)? AND Person2 parent(Parent2)? AND Parent2 parent(Grandparent)?'";
      expression = "Person1 parent(Parent1)? AND Parent1 parent(Grandparent)? AND Person2 parent(Parent2)? AND Parent2 parent(Grandparent)?";
      rule = this:parse_expression(expression, 'find_cousins);
      "Build family tree:";
      "  $arch_wizard (grandparent)";
      "    |";
      "    +-- mother1_obj (parent of person1)";
      "    |      |";
      "    |      +-- person1_obj (ego)";
      "    |";
      "    +-- mother2_obj (parent of person2)";
      "           |";
      "           +-- person2_obj (cousin)";
      person1_obj = #64:create(false);
      person1_obj.father = #0;
      person1_obj.mother = #0;
      mother1_obj = #64:create(false);
      person1_obj.mother = mother1_obj;
      mother1_obj.father = $arch_wizard;
      mother2_obj = #64:create(false);
      mother2_obj.father = $arch_wizard;
      person2_obj = #64:create(false);
      person2_obj.father = mother2_obj;
      "Evaluate with: Person1=person1_obj, Person2=person2_obj";
      "This should find: Parent1=mother1_obj, Parent2=mother2_obj, Grandparent=$arch_wizard";
      bindings = ['Person1 -> person1_obj, 'Person2 -> person2_obj];
      result = this:evaluate(rule, bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should find shared grandparent");
      result_bindings = result['bindings];
      result_bindings['Grandparent] == $arch_wizard || raise(E_ASSERT, "Grandparent should be $arch_wizard");
    finally
      valid(person2_obj) && person2_obj:destroy();
      valid(mother2_obj) && mother2_obj:destroy();
      valid(mother1_obj) && mother1_obj:destroy();
      valid(person1_obj) && person1_obj:destroy();
    endtry
    return true;
  endmethod

  method test_ancestor_chain owner: ARCH_WIZARD
    "Test longer ancestor chain: 4-generation ancestor query.";
    test_obj = #-1;
    mother_obj = #-1;
    grandmother_obj = #-1;
    great_grandmother_obj = #-1;
    great_great_grandmother_obj = #-1;
    try
      "Build 5-generation family to test 4-step chain";
      test_obj = #64:create(false);
      test_obj.father = #0;
      test_obj.mother = #0;
      "Create mother (gen 2)";
      mother_obj = #64:create(false);
      test_obj.mother = mother_obj;
      "Create maternal grandmother (gen 3)";
      grandmother_obj = #64:create(false);
      mother_obj.mother = grandmother_obj;
      "Create maternal great-grandmother (gen 4)";
      great_grandmother_obj = #64:create(false);
      grandmother_obj.mother = great_grandmother_obj;
      "Create maternal great-great-grandmother (gen 5)";
      great_great_grandmother_obj = #64:create(false);
      great_grandmother_obj.mother = great_great_grandmother_obj;
      "Query: 4-goal chain to find great-great-grandmother";
      "This tests: test_obj -> gen2 -> gen3 -> gen4 -> gen5";
      goals = {{'parent, test_obj, {'var, 'P1}}, {'parent, {'var, 'P1}, {'var, 'P2}}, {'parent, {'var, 'P2}, {'var, 'P3}}, {'parent, {'var, 'P3}, {'var, 'Result}}};
      empty_bindings = [];
      result = this:_prove_goals(goals, empty_bindings);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      result['success] || raise(E_ASSERT, "Should find 4-generation ancestor chain");
      bindings = result['bindings];
      bindings['Result] == great_great_grandmother_obj || raise(E_ASSERT, "Should find great-great-grandmother through 4-step chain");
    finally
      valid(great_great_grandmother_obj) && great_great_grandmother_obj:destroy();
      valid(great_grandmother_obj) && great_grandmother_obj:destroy();
      valid(grandmother_obj) && grandmother_obj:destroy();
      valid(mother_obj) && mother_obj:destroy();
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_parse_negation owner: ARCH_WIZARD
    "Test parsing negation (NOT) operator.";
    "Expression: 'NOT Object predicate?'";
    expression = "NOT player has_magic?";
    rule = this:parse_expression(expression, 'test_not);
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Should return flyweight");
    rule.body != {} || raise(E_ASSERT, "Body should not be empty");
    "Body should contain a NOT goal: {{'not, {...}}}";
    body = rule.body;
    length(body) > 0 || raise(E_ASSERT, "Body should have at least one goal");
    first_goal = body[1];
    typeof(first_goal) == TYPE_LIST || raise(E_ASSERT, "First goal should be list");
    length(first_goal) > 0 || raise(E_ASSERT, "First goal should have content");
    first_goal[1] == 'not || raise(E_ASSERT, "First goal should start with 'not marker");
    return true;
  endmethod

  method test_negation_succeeds owner: ARCH_WIZARD
    "Test negation as failure: NOT succeeds when inner goal fails.";
    test_obj = #64;
    "Create a goal that will fail (reputation >= 100)";
    goal_that_fails = {'reputation, test_obj, 100};
    "Wrap it in NOT: {{'not, goal_that_fails}}";
    not_goal = {'not, goal_that_fails};
    goals = {not_goal};
    empty_bindings = [];
    result = this:_prove_goals(goals, empty_bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    result['success] || raise(E_ASSERT, "NOT of failing goal should succeed");
    return true;
  endmethod

  method test_negation_fails owner: ARCH_WIZARD
    "Test negation as failure: NOT fails when inner goal succeeds.";
    test_obj = #64;
    "Create a goal that will succeed (reputation >= 5)";
    goal_that_succeeds = {'reputation, test_obj, 5};
    "Wrap it in NOT: {{'not, goal_that_succeeds}}";
    not_goal = {'not, goal_that_succeeds};
    goals = {not_goal};
    empty_bindings = [];
    result = this:_prove_goals(goals, empty_bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    !result['success] || raise(E_ASSERT, "NOT of succeeding goal should fail");
    return true;
  endmethod

  method test_negation_with_conjunction owner: ARCH_WIZARD
    "Test negation in conjunction: goal1 AND NOT goal2.";
    test_obj = #64;
    "Create: reputation(test_obj, 5) AND NOT reputation(test_obj, 100)";
    "Both should succeed: first because 8 >= 5, second because 8 < 100";
    goal1 = {'reputation, test_obj, 5};
    goal2_fails = {'reputation, test_obj, 100};
    not_goal = {'not, goal2_fails};
    goals = {goal1, not_goal};
    empty_bindings = [];
    result = this:_prove_goals(goals, empty_bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    result['success] || raise(E_ASSERT, "goal1 AND NOT goal2 should succeed");
    return true;
  endmethod

  method test_parse_not_failure_expression owner: ARCH_WIZARD
    "Test parsing and evaluating NOT expression from string.";
    "Expression: 'this reputation(5)? AND NOT this reputation(100)?'";
    test_obj = #64;
    expression = "this reputation(5)? AND NOT this reputation(100)?";
    rule = this:parse_expression(expression, 'test_not_parsed);
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Should parse to flyweight");
    rule.body != {} || raise(E_ASSERT, "Body should not be empty");
    "Evaluate with initial binding this -> test_obj";
    bindings = ['this -> test_obj];
    result = this:evaluate(rule, bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    "First goal succeeds (8 >= 5), NOT goal succeeds (8 < 100), so conjunction succeeds";
    result['success] || raise(E_ASSERT, "Expression should succeed");
    return true;
  endmethod

  method test_parse_complex_not_expression owner: ARCH_WIZARD
    "Test complex NOT expression: 'this reputation(5)? AND NOT this reputation(100)? OR this reputation(3)?'";
    test_obj = #64;
    "This should parse to: (reputation(5) AND NOT reputation(100)) OR reputation(3)";
    expression = "this reputation(5)? AND NOT this reputation(100)? OR this reputation(3)?";
    rule = this:parse_expression(expression, 'test_not_complex);
    typeof(rule) == TYPE_FLYWEIGHT || raise(E_ASSERT, "Should parse to flyweight");
    "Evaluate with initial binding this -> test_obj";
    bindings = ['this -> test_obj];
    result = this:evaluate(rule, bindings);
    typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
    "First branch: reputation(5) AND NOT reputation(100) should succeed (8 >= 5, 8 < 100)";
    result['success] || raise(E_ASSERT, "Expression should succeed");
    return true;
  endmethod

  method test_negation_bounded_one_unbound owner: ARCH_WIZARD
    "Test that bounded negation (1 unbound var) is allowed.";
    test_obj = #-1;
    try
      test_obj = #64:create(false);
      "Create a rule: NOT parent(test_obj, Parent) where Parent is unbound";
      "This is bounded negation - one unbound variable is OK";
      "Set up test_obj to have a father";
      test_obj.father = $root;
      not_goal = {'not, {'parent, test_obj, {'var, 'Parent}}};
      rule = <$rule, .name = 'test_bounded_not, .head = 'test_bounded_not, .body = {not_goal}, .variables = {'Parent}>;
      "Evaluate with no bindings - should NOT have errors";
      result = this:evaluate(rule, []);
      typeof(result) == TYPE_MAP || raise(E_ASSERT, "Result should be map");
      "Check for ERROR messages (warnings are OK, but errors mean 2+ unbound)";
      has_error = false;
      for warning in (result['warnings])
        if (index(warning, "ERROR:") > 0)
          has_error = true;
          break;
        endif
      endfor
      has_error && raise(E_ASSERT, "Should NOT have ERROR for 1 unbound variable");
      "NOT parent(test_obj, Parent) checks if there exists any parent";
      "Since test_obj has father = $root, fact_parent will return solutions";
      "So NOT should fail";
      !result['success] || raise(E_ASSERT, "NOT parent should fail (object has parents)");
    finally
      valid(test_obj) && test_obj:destroy();
    endtry
    return true;
  endmethod

  method test_decompile_simple owner: ARCH_WIZARD
    "Test decompiling a simple rule back to DSL.";
    test_obj = #64;
    "Parse an expression";
    expr = "this reputation(5)?";
    rule = this:parse_expression(expr, 'test_decomp);
    "Decompile it back";
    result = this:decompile_rule(rule);
    typeof(result) == TYPE_STR || raise(E_ASSERT, "Result should be string");
    "Result should contain the key parts";
    index(result, "this") > 0 || raise(E_ASSERT, "Should contain 'this'");
    index(result, "reputation") > 0 || raise(E_ASSERT, "Should contain 'reputation'");
    index(result, "5") > 0 || raise(E_ASSERT, "Should contain '5'");
    return true;
  endmethod

  method test_decompile_and owner: ARCH_WIZARD
    "Test decompiling AND expressions.";
    expr = "this reputation(5)? AND this reputation(3)?";
    rule = this:parse_expression(expr, 'test_and_decomp);
    result = this:decompile_rule(rule);
    typeof(result) == TYPE_STR || raise(E_ASSERT, "Result should be string");
    index(result, "AND") > 0 || raise(E_ASSERT, "Should contain 'AND'");
    index(result, "reputation") > 0 || raise(E_ASSERT, "Should contain 'reputation'");
    return true;
  endmethod

  method test_decompile_or owner: ARCH_WIZARD
    "Test decompiling OR expressions.";
    expr = "this reputation(5)? OR this reputation(3)?";
    rule = this:parse_expression(expr, 'test_or_decomp);
    result = this:decompile_rule(rule);
    typeof(result) == TYPE_STR || raise(E_ASSERT, "Result should be string");
    index(result, "OR") > 0 || raise(E_ASSERT, "Should contain 'OR'");
    index(result, "reputation") > 0 || raise(E_ASSERT, "Should contain 'reputation'");
    return true;
  endmethod

  method test_decompile_not owner: ARCH_WIZARD
    "Test decompiling NOT expressions.";
    expr = "NOT this reputation(100)?";
    rule = this:parse_expression(expr, 'test_not_decomp);
    result = this:decompile_rule(rule);
    typeof(result) == TYPE_STR || raise(E_ASSERT, "Result should be string");
    index(result, "NOT") > 0 || raise(E_ASSERT, "Should contain 'NOT'");
    index(result, "reputation") > 0 || raise(E_ASSERT, "Should contain 'reputation'");
    return true;
  endmethod

  method test_decompile_complex owner: ARCH_WIZARD
    "Test decompiling complex expressions.";
    expr = "this reputation(5)? AND NOT this reputation(100)? OR this reputation(3)?";
    rule = this:parse_expression(expr, 'test_complex_decomp);
    result = this:decompile_rule(rule);
    typeof(result) == TYPE_STR || raise(E_ASSERT, "Result should be string");
    index(result, "OR") > 0 || raise(E_ASSERT, "Should contain 'OR'");
    index(result, "AND") > 0 || raise(E_ASSERT, "Should contain 'AND'");
    index(result, "NOT") > 0 || raise(E_ASSERT, "Should contain 'NOT'");
    return true;
  endmethod

  method test_container_access_public owner: ARCH_WIZARD
    "Test that containers with no rule allow public access.";
    chest = $container:create(true);
    sword = $thing:create(true);
    sword:moveto(chest);
    player_obj = $player:create(true);
    "No rule = public access";
    check = chest:can_take_from(player_obj, sword);
    check['allowed] || raise(E_ASSERT, "Should allow access with no rule");
    check = chest:can_put_into(player_obj, sword);
    check['allowed] || raise(E_ASSERT, "Should allow put with no rule");
    return true;
  endmethod

  method test_container_access_owner_only owner: ARCH_WIZARD
    "Test owner-only container access.";
    chest = $container:create(true);
    sword = $thing:create(true);
    sword:moveto(chest);
    owner = $player:create(true);
    other = $player:create(true);
    chest.owner = owner;
    "Set owner-only take rule";
    chest.take_rule = this:parse_expression("This owner_is(Accessor)?", 'owner_only);
    "Owner should succeed";
    check = chest:can_take_from(owner, sword);
    check['allowed] || raise(E_ASSERT, "Owner should have access");
    "Non-owner should fail";
    check = chest:can_take_from(other, sword);
    !check['allowed] || raise(E_ASSERT, "Non-owner should be denied");
    typeof(check['reason]) == TYPE_LIST || raise(E_ASSERT, "Should have denial reason");
    return true;
  endmethod

  method test_container_access_wizard_bypass owner: ARCH_WIZARD
    "Test that wizards can bypass container rules.";
    chest = $container:create(true);
    sword = $thing:create(true);
    sword:moveto(chest);
    owner = $player:create(true);
    wizard_player = $player:create(true);
    wizard_player.wizard = true;
    chest.owner = owner;
    "Set owner-only rule";
    chest.take_rule = this:parse_expression("This owner_is(Accessor)? OR Accessor is_wizard?", 'owner_or_wizard);
    "Wizard should bypass";
    check = chest:can_take_from(wizard_player, sword);
    check['allowed] || raise(E_ASSERT, "Wizard should bypass with OR accessor is_wizard?");
    return true;
  endmethod

  method test_container_asymmetric_rules owner: ARCH_WIZARD
    "Test asymmetric access (different take and put rules).";
    donation_box = $container:create(true);
    coin = $thing:create(true);
    owner = $player:create(true);
    donor = $player:create(true);
    donation_box.owner = owner;
    coin.owner = donor;
    "Anyone can put (donate)";
    donation_box.put_rule = 0;
    "Only owner can take";
    donation_box.take_rule = this:parse_expression("This owner_is(Accessor)?", 'owner_take);
    "Donor can put";
    check = donation_box:can_put_into(donor, coin);
    check['allowed] || raise(E_ASSERT, "Donor should be able to donate");
    "But donor cannot take";
    coin:moveto(donation_box);
    check = donation_box:can_take_from(donor, coin);
    !check['allowed] || raise(E_ASSERT, "Donor should not be able to take back");
    "Owner can take";
    check = donation_box:can_take_from(owner, coin);
    check['allowed] || raise(E_ASSERT, "Owner should be able to take donations");
    return true;
  endmethod

  method test_object_literals_in_rules owner: ARCH_WIZARD
    "Test that object literals (both #num and UUID formats) work in rule expressions";
    "Create test container and item";
    chest = #-1;
    sword = #-1;
    try
      chest = $container:create();
      sword = $thing:create();
      sword:moveto(chest);
      "Test with object literal (tostr already includes #)";
      obj_str = tostr(sword);
      chest.take_rule = this:parse_expression("This contains(" + obj_str + ")?", 'obj_literal);
      result = $rule_engine:evaluate(chest.take_rule, ['This -> chest]);
      result['success] || raise(E_ASSERT, "Object literal should work: " + obj_str);
    finally
      valid(sword) && sword:destroy();
      valid(chest) && chest:destroy();
    endtry
    return true;
  endmethod
endobject
