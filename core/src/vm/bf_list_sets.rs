use std::sync::Arc;

use async_trait::async_trait;

use moor_values::var::error::Error::{E_INVARG, E_TYPE};
use moor_values::var::variant::Variant;
use moor_values::var::{v_empty_list, v_int, v_list, v_string};
use regexpr_binding::Pattern;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::vm::builtin::BfRet::{Error, Ret};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::vm_execute::one_to_zero_index;
use crate::vm::VM;

async fn bf_is_member<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let (value, list) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(Error(E_TYPE));
    };
    if list.contains_case_sensitive(value) {
        Ok(Ret(v_int(1)))
    } else {
        Ok(Ret(v_int(0)))
    }
}
bf_declare!(is_member, bf_is_member);

async fn bf_listinsert<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(Error(E_INVARG));
    }
    let (list, value) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(Error(E_TYPE));
    };
    let new_list = if bf_args.args.len() == 2 {
        list.push(value)
    } else {
        let index = match one_to_zero_index(&bf_args.args[2]) {
            Ok(i) => i,
            Err(e) => return Ok(Error(e)),
        };
        list.insert(index as isize, value)
    };
    Ok(Ret(new_list))
}
bf_declare!(listinsert, bf_listinsert);

async fn bf_listappend<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(Error(E_INVARG));
    }
    let (list, value) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant().clone() else {
        return Ok(Error(E_TYPE));
    };
    let new_list = if bf_args.args.len() == 2 {
        list.push(value)
    } else {
        let index = bf_args.args[2].variant();
        let Variant::Int(index) = index else {
            return Ok(Error(E_TYPE));
        };
        list.insert(*index as isize, value)
    };
    Ok(Ret(new_list))
}
bf_declare!(listappend, bf_listappend);

async fn bf_listdelete<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let (list, index) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(Error(E_TYPE));
    };
    let index = match one_to_zero_index(index) {
        Ok(i) => i,
        Err(e) => return Ok(Error(e)),
    };
    Ok(Ret(list.remove_at(index as usize)))
}
bf_declare!(listdelete, bf_listdelete);

async fn bf_listset<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let (list, value) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(Error(E_TYPE));
    };
    let index = match one_to_zero_index(&bf_args.args[2]) {
        Ok(i) => i,
        Err(e) => return Ok(Error(e)),
    };
    Ok(Ret(list.set(index as usize, value)))
}
bf_declare!(listset, bf_listset);

async fn bf_setadd<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let (list, value) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(Error(E_TYPE));
    };
    if !list.contains(value) {
        return Ok(Ret(list.push(value)));
    }
    Ok(Ret(bf_args.args[0].clone()))
}
bf_declare!(setadd, bf_setadd);

async fn bf_setremove<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let (list, value) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(Error(E_TYPE));
    };
    Ok(Ret(list.setremove(value)))
}
bf_declare!(setremove, bf_setremove);

#[no_mangle]
#[used]
// TODO: This is not thread safe. If we actually want to use this flag, we will want to put the
// whole 'legacy' regex engine in a mutex.
pub static mut task_timed_out: u64 = 0;

async fn bf_match<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(Error(E_INVARG));
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Ok(Error(E_TYPE)),
    };

    let case_matters = if bf_args.args.len() == 3 {
        let Variant::Int(case_matters) = bf_args.args[2].variant() else {
            return Ok(Error(E_TYPE));
        };
        *case_matters == 1
    } else {
        false
    };

    // TODO: pattern cache?
    let Ok(pattern) = Pattern::new(pattern.as_str(), case_matters) else {
        return Ok(Error(E_INVARG));
    };

    let Ok((overall, match_vec)) = pattern.match_pattern(subject.as_str()) else {
        return Ok(Ret(v_empty_list()));
    };

    let subs = v_list(
        match_vec
            .iter()
            .map(|(start, end)| v_list(vec![v_int(*start as i64), v_int(*end as i64)]))
            .collect(),
    );
    Ok(Ret(v_list(vec![
        v_int(overall.0 as i64),
        v_int(overall.1 as i64),
        subs,
        bf_args.args[0].clone(),
    ])))
}
bf_declare!(match, bf_match);

async fn bf_rmatch<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(Error(E_INVARG));
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Ok(Error(E_TYPE)),
    };

    let case_matters = if bf_args.args.len() == 3 {
        let Variant::Int(case_matters) = bf_args.args[2].variant() else {
            return Ok(Error(E_TYPE));
        };
        *case_matters == 1
    } else {
        false
    };

    // TODO: pattern cache?
    let Ok(pattern) = Pattern::new(pattern.as_str(), case_matters) else {
        return Ok(Error(E_INVARG));
    };

    let Ok((overall, match_vec)) = pattern.reverse_match_pattern(subject.as_str()) else {
        return Ok(Ret(v_empty_list()));
    };

    let subs = v_list(
        match_vec
            .iter()
            .map(|(start, end)| v_list(vec![v_int(*start as i64), v_int(*end as i64)]))
            .collect(),
    );
    Ok(Ret(v_list(vec![
        v_int(overall.0 as i64),
        v_int(overall.1 as i64),
        subs,
        bf_args.args[0].clone(),
    ])))
}
bf_declare!(rmatch, bf_rmatch);

fn substitute(
    template: &str,
    subs: &[(isize, isize)],
    source: &str,
) -> Result<String, moor_values::var::error::Error> {
    // textual patterns of form %<int> (e.g. %1, %9, %11) are replaced by the text matched by the
    // offsets (1-indexed) into source given by the corresponding value in `subs`.

    // We'll append to this result.
    let mut result = String::new();

    // Then char-by-char iterate through `source`; if we see a %, we'll start lexing a # until we
    // see a non-digit, then we'll parse the number and look it up in `subs`.
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            // We've seen a non-%, so we'll just append it to `result`.
            result.push(c);
            continue;
        }

        // We've seen a %, so we'll start lexing a number. But if the next char is a %, we'll
        // just append a % to `result` and continue.
        let mut number = String::new();
        let mut last_c = None;
        for c in chars.by_ref() {
            if c.is_ascii_digit() {
                number.push(c);
            } else {
                // We've seen a non-digit, so we'll stop lexing, but keep the character to append
                // after our substitution.
                last_c = Some(c);
                break;
            }
        }
        // Now we'll parse the number.
        let Ok(number) = number.parse::<usize>() else {
            // If we can't parse the number, we'll raise an error.
            return Err(E_INVARG);
        };

        // If the number is out of range, we'll raise an E_INVARG. E_RANGE would be nice, but
        // that's not what MOO does.
        if number > subs.len() {
            return Err(E_INVARG);
        }

        // We're 1-indexed, so we'll subtract 1 from the number.
        let number = number - 1;

        // And look it up in `subs`.
        let (start, end) = (subs[number].0, subs[number].1);

        // Now validate the range in the source string, and raise an E_INVARG if it's invalid.
        if start < 0 || start > end || end >= (source.len() as isize) {
            return Err(E_INVARG);
        }

        let (start, end) = (start as usize - 1, end as usize);
        // Now append the corresponding substring to `result`.
        result.push_str(&source[start..end]);
        if let Some(last_c) = last_c {
            result.push(last_c);
        }
    }
    Ok(result)
}

async fn bf_substitute<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let (template, subs) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(template), Variant::List(subs)) => (template, subs),
        _ => return Ok(Error(E_TYPE)),
    };

    // Subs is of form {<start>, <end>, <replacements>, <subject>}
    // "replacement" and subject are what we're interested in.
    if subs.len() != 4 {
        return Ok(Error(E_INVARG));
    }

    let (Variant::List(subs), Variant::Str(source)) = (subs[2].variant(), subs[3].variant()) else {
        return Ok(Error(E_INVARG));
    };

    // Turn psubs into a Vec<(isize, isize)>. Raising errors on the way if they're not
    let mut mysubs = Vec::new();
    for sub in &subs[..] {
        let Variant::List(sub) = sub.variant() else {
            return Ok(Error(E_INVARG));
        };
        if sub.len() != 2 {
            return Ok(Error(E_INVARG));
        }
        let (Variant::Int(start), Variant::Int(end)) = (sub[0].variant(), sub[1].variant()) else {
            return Ok(Error(E_INVARG));
        };
        mysubs.push((*start as isize, *end as isize));
    }

    match substitute(template.as_str(), &mysubs, source.as_str()) {
        Ok(r) => Ok(Ret(v_string(r))),
        Err(e) => Ok(Error(e)),
    }
}
bf_declare!(substitute, bf_substitute);

impl VM {
    pub(crate) fn register_bf_list_sets(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("is_member")] = Arc::new(Box::new(BfIsMember {}));
        self.builtins[offset_for_builtin("listinsert")] = Arc::new(Box::new(BfListinsert {}));
        self.builtins[offset_for_builtin("listappend")] = Arc::new(Box::new(BfListappend {}));
        self.builtins[offset_for_builtin("listdelete")] = Arc::new(Box::new(BfListdelete {}));
        self.builtins[offset_for_builtin("listset")] = Arc::new(Box::new(BfListset {}));
        self.builtins[offset_for_builtin("setadd")] = Arc::new(Box::new(BfSetadd {}));
        self.builtins[offset_for_builtin("setremove")] = Arc::new(Box::new(BfSetremove {}));
        self.builtins[offset_for_builtin("match")] = Arc::new(Box::new(BfMatch {}));
        self.builtins[offset_for_builtin("rmatch")] = Arc::new(Box::new(BfRmatch {}));
        self.builtins[offset_for_builtin("substitute")] = Arc::new(Box::new(BfSubstitute {}));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use regexpr_binding::Pattern;

    use crate::vm::bf_list_sets::substitute;

    #[test]
    fn test_match_substitute() {
        let pattern = Pattern::new("%(%w*%) to %(%w*%)", false).unwrap();
        let source = "*** Welcome to LambdaMOO!!!";
        let (overall, subs) = pattern.match_pattern(source).unwrap();
        assert_eq!(overall, (5, 24));
        assert_eq!(
            subs,
            vec![
                (5, 11),
                (16, 24),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1)
            ]
        );
        let result = substitute("I thank you for your %1 here in %2.", &subs, source).unwrap();
        assert_eq!(result, "I thank you for your Welcome here in LambdaMOO.");
    }

    #[test]
    fn test_substitute_off_by_one() {
        let pattern =
            Pattern::new("^@%([^-]*%)%(o%|opt?i?o?n?s?%|-o?p?t?i?o?n?s?%)$", false).unwrap();
        let source = "@edit-o";
        let (overall, subs) = pattern.match_pattern(source).unwrap();
        assert_eq!(overall, (1, 7));
        assert_eq!(
            subs,
            vec![
                (2, 5),
                (6, 7),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
                (0, -1),
            ]
        );
        let result = substitute("%1", &subs, source).unwrap();
        assert_eq!(result, "edit");
    }
}
