object RULE [
  import_export_id -> "rule",
  import_export_hierarchy -> {"rules"}
]
  name: "Rule"
  parent: ROOT
  owner: ARCH_WIZARD
  readable: true

  override description (owner: ARCH_WIZARD, flags: "rc") = "A validated condition tree with declared input, parameter, and state types.";

  method mk owner: ARCH_WIZARD
    "Construct one rule from a canonical condition tree and its declared scope.";
    set_task_perms(caller_perms());
    const {name, condition, ?scope = $rule_engine:scope(), ?parameters = []} = args;
    const tree = $condition:prepare_relations(condition, parameters);
    const rule = <$rule, .name = tosym(name), .condition = tree, .scope = scope>;
    const validation = $rule_engine:validate_rule(rule);
    validation['valid] || raise(E_INVARG, validation['warnings]:join("; "));
    return rule;
  endmethod

  method evaluate owner: ARCH_WIZARD
    "Evaluate with explicit input values, parameters, and local state under caller authority.";
    set_task_perms(caller_perms());
    const {?inputs = [], ?parameters = [], ?state = []} = args;
    return $rule_engine:evaluate(this, inputs, parameters, state);
  endmethod
endobject
