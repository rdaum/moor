//! Tests for operand parsing fixes (builtin_call and sysprop)

use crate::{CompileOptions, compile};

#[test]
fn test_builtin_call_in_scatter_assignment() {
    let code = "{_, _, perms, @_} = callers()[2];";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "builtin_call in scatter assignment should compile");
}

#[test]
fn test_builtin_call_in_simple_assignment() {
    let code = "result = callers()[2];";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "builtin_call in simple assignment should compile");
}

#[test]
fn test_builtin_call_with_args() {
    let code = "x = length(args);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "builtin_call with args should compile");
}

#[test]
fn test_builtin_call_no_args_in_scatter() {
    let code = "{a, b} = time();";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "builtin_call with no args in scatter should compile");
}

#[test]
fn test_sysprop_call_in_scatter_assignment() {
    let code = "{msg, parties} = $pronoun_sub:flatten_message(msg, parties);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "sysprop_call in scatter assignment should compile");
}

#[test]
fn test_sysprop_call_in_simple_assignment() {
    let code = "result = $pronoun_sub:flatten_message(msg, parties);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "sysprop_call in simple assignment should compile");
}

#[test]
fn test_sysprop_in_simple_assignment() {
    let code = "x = $some_prop;";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "sysprop in simple assignment should compile");
}

#[test]
fn test_sysprop_in_scatter_assignment() {
    let code = "{a, b} = $some_prop;";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "sysprop in scatter assignment should compile");
}

#[test]
fn test_builtin_call_plus_sysprop_in_expression() {
    let code = "result = callers()[2] + $some_prop;";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "builtin_call + sysprop in expression should compile");
}

#[test]
fn test_mixed_operand_types() {
    let code = "{a, b} = $obj:verb(callers()[1]);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "mixed operand types should compile");
}

#[test]
fn test_jhcore_builtin_context() {
    let code = "parties = $pronoun_sub:parse_parties(p_s_args, caller);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "original JHCore builtin context should compile");
}

#[test]
fn test_jhcore_sysprop_context() {
    let code = "tell = $string_utils:pronoun_sub(msg, @parties);";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "original JHCore sysprop context should compile");
}

#[test]
fn test_nested_system_calls() {
    let code = "party_set = $set_utils:union(@$list_utils:slice(parties));";
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "nested system calls should compile");
}

#[test]
fn test_jhcore_object16_verb6() {
    // The original problematic verb from JHCore object #16, verb 6
    let code = r#"
{_, _, perms, @_} = callers()[2];
return !perms.wizard;
"#;
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "JHCore object #16 verb 6 should compile");
}

#[test]
fn test_jhcore_object34_verb0() {
    // The original problematic verb from JHCore object #34, verb 0
    let code = r#"
msg = args[1];
p_s_args = args[2..length(args)];
parties = $pronoun_sub:parse_parties(p_s_args, caller);
wheres = parties[3][1];
{msg, parties} = $pronoun_sub:flatten_message(msg, parties);
tell = $string_utils:pronoun_sub(msg, @parties);
party_set = $set_utils:union(@$list_utils:slice(parties));
for where in (wheres)
`where:announce_all_but(party_set, tell) ! E_VERBNF';
endfor
for p in (party_set)
this:say(msg, p, parties);
endfor
"#;
    let result = compile(code, CompileOptions::default());
    assert!(result.is_ok(), "JHCore object #34 verb 0 should compile");
}