// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use std::ops::BitOr;
use std::sync::Arc;

use async_trait::async_trait;
use onig::{Region, SearchOptions, SyntaxOperator};

use moor_compiler::offset_for_builtin;
use moor_values::var::Error::{E_INVARG, E_TYPE};
use moor_values::var::Variant;
use moor_values::var::{v_empty_list, v_int, v_list, v_string};
use moor_values::var::{v_listv, Error};

use crate::bf_declare;
use crate::builtins::BfRet::Ret;
use crate::builtins::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::vm_execute::one_to_zero_index;
use crate::vm::VM;

async fn bf_is_member<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let (value, list) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Err(E_TYPE);
    };
    if list.contains_case_sensitive(value) {
        Ok(Ret(v_int(1)))
    } else {
        Ok(Ret(v_int(0)))
    }
}
bf_declare!(is_member, bf_is_member);

async fn bf_listinsert<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(E_INVARG);
    }
    let len = bf_args.args.len();
    let value = bf_args.args[1].clone();
    if len == 2 {
        let list = &mut bf_args.args[0];
        let Variant::List(list) = list.variant_mut() else {
            return Err(E_TYPE);
        };
        Ok(Ret(list.push(value)))
    } else {
        let index = bf_args.args[2].clone();
        let list = &mut bf_args.args[0];
        let Variant::List(list) = list.variant_mut() else {
            return Err(E_TYPE);
        };
        let index = match one_to_zero_index(&index) {
            Ok(i) => i,
            Err(e) => return Err(e),
        };
        Ok(Ret(list.insert(index as isize, value)))
    }
}
bf_declare!(listinsert, bf_listinsert);

async fn bf_listappend<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(E_INVARG);
    }
    let value = bf_args.args[1].clone();
    let list = &mut bf_args.args[0];
    let Variant::List(mut list) = list.variant_mut().clone() else {
        return Err(E_TYPE);
    };
    let new_list = if bf_args.args.len() == 2 {
        list.push(value.clone())
    } else {
        let index = bf_args.args[2].variant();
        let Variant::Int(index) = index else {
            return Err(E_TYPE);
        };
        list.insert(*index as isize, value.clone())
    };
    Ok(Ret(new_list))
}
bf_declare!(listappend, bf_listappend);

async fn bf_listdelete<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let index = bf_args.args[1].clone();
    let list = bf_args.args[0].variant_mut();
    let Variant::List(list) = list else {
        return Err(E_TYPE);
    };
    let index = match one_to_zero_index(&index) {
        Ok(i) => i,
        Err(e) => return Err(e),
    };
    Ok(Ret(list.remove_at(index)))
}
bf_declare!(listdelete, bf_listdelete);

async fn bf_listset<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 3 {
        return Err(E_INVARG);
    }
    let index = bf_args.args[2].clone();
    let value = bf_args.args[1].clone();
    let list = &mut bf_args.args[0];
    let Variant::List(ref mut list) = list.variant_mut() else {
        return Err(E_TYPE);
    };
    let index = match one_to_zero_index(&index) {
        Ok(i) => i,
        Err(e) => return Err(e),
    };
    Ok(Ret(list.set(index as usize, value.clone())))
}
bf_declare!(listset, bf_listset);

async fn bf_setadd<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let value = bf_args.args[1].clone();
    let list = &mut bf_args.args[0];
    let Variant::List(ref mut list) = list.variant_mut() else {
        return Err(E_TYPE);
    };
    if !list.contains(&value) {
        return Ok(Ret(list.push(value.clone())));
    }
    Ok(Ret(bf_args.args[0].clone()))
}
bf_declare!(setadd, bf_setadd);

async fn bf_setremove<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let value = bf_args.args[1].clone();
    let list = bf_args.args[0].variant_mut();
    let Variant::List(ref mut list) = list else {
        return Err(E_TYPE);
    };
    Ok(Ret(list.setremove(&value)))
}
bf_declare!(setremove, bf_setremove);

#[no_mangle]
#[used]
// TODO: This is not thread safe. If we actually want to use this flag, we will want to put the
// whole 'legacy' regex engine in a mutex.
pub static mut task_timed_out: u64 = 0;

/// Translate a MOO pattern into a more standard syntax.  Effectively, this
/// just involves remove `%' escapes into `\' escapes.
fn translate_pattern(pattern: &str) -> Option<String> {
    let mut s = String::with_capacity(pattern.len());
    let mut c_iter = pattern.chars();
    loop {
        let Some(mut c) = c_iter.next() else {
            break;
        };
        if c == '%' {
            let Some(escape) = c_iter.next() else {
                return None;
            };
            if ".*+?[^$|()123456789bB<>wW".contains(escape) {
                s.push('\\');
            }
            s.push(escape);
            continue;
        }
        if c == '\\' {
            s.push_str("\\\\");
            continue;
        }
        if c == '[' {
            /* Any '%' or '\' characters inside a charset should be copied
             * over without translation. */
            s.push(c);
            let Some(next) = c_iter.next() else {
                return None;
            };
            c = next;
            if c == '^' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            if c == ']' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            while c != ']' {
                s.push(c);
                let Some(next) = c_iter.next() else {
                    return None;
                };
                c = next;
            }
            s.push(c);
            continue;
        }
        s.push(c);
    }
    Some(s)
}

type Span = (isize, isize);
type MatchSpans = (Span, Vec<Span>);

fn perform_regex_match(
    pattern: &str,
    subject: &str,
    case_matters: bool,
    reverse: bool,
) -> Result<Option<MatchSpans>, Error> {
    let Some(translated_pattern) = translate_pattern(pattern) else {
        return Err(E_INVARG);
    };

    let options = if case_matters {
        onig::RegexOptions::REGEX_OPTION_NONE
    } else {
        onig::RegexOptions::REGEX_OPTION_IGNORECASE
    };

    let mut syntax = *onig::Syntax::grep();
    syntax.set_operators(
        syntax
            .operators()
            .bitor(SyntaxOperator::SYNTAX_OPERATOR_QMARK_ZERO_ONE)
            .bitor(SyntaxOperator::SYNTAX_OPERATOR_PLUS_ONE_INF),
    );
    let regex = onig::Regex::with_options(translated_pattern.as_str(), options, &syntax)
        .map_err(|_| E_INVARG)?;

    let (search_start, search_end) = if reverse {
        (subject.len(), 0)
    } else {
        (0, subject.len())
    };
    let mut region = Region::new();

    let Some(_) = regex.search_with_options(
        subject,
        search_start,
        search_end,
        SearchOptions::SEARCH_OPTION_NONE,
        Some(&mut region),
    ) else {
        return Ok(None);
    };
    // Overall span
    let Some((start, end)) = region.pos(0) else {
        return Ok(None);
    };

    let overall = ((start + 1) as isize, end as isize);
    // Now we'll iterate through the captures, and build up a Vec<Span> of the captured groups.
    // MOO match() returns 9 subpatterns, no more, no less. So we start with a Vec of 9
    // (-1, -1) pairs and then fill that in with the captured groups, if any.
    let mut match_vec = vec![(0, -1); 9];
    for i in 1..=8 {
        if let Some((start, end)) = region.pos(i) {
            match_vec[i - 1] = ((start + 1) as isize, end as isize);
        }
    }

    Ok(Some((overall, match_vec)))
}

/// Common code for both match and rmatch.
fn do_re_match(bf_args: &mut BfCallState<'_>, reverse: bool) -> Result<BfRet, Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(E_INVARG);
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Err(E_TYPE),
    };

    let case_matters = if bf_args.args.len() == 3 {
        let Variant::Int(case_matters) = bf_args.args[2].variant() else {
            return Err(E_TYPE);
        };
        *case_matters == 1
    } else {
        false
    };

    // TODO: pattern cache?
    let Some((overall, match_vec)) =
        perform_regex_match(pattern.as_str(), subject.as_str(), case_matters, reverse)?
    else {
        return Ok(Ret(v_empty_list()));
    };

    let subs = v_listv(
        match_vec
            .iter()
            .map(|(start, end)| v_list(&[v_int(*start as i64), v_int(*end as i64)]))
            .collect::<Vec<_>>(),
    );
    Ok(Ret(v_list(&[
        v_int(overall.0 as i64),
        v_int(overall.1 as i64),
        subs,
        bf_args.args[0].clone(),
    ])))
}
async fn bf_match<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    do_re_match(bf_args, false)
}
bf_declare!(match, bf_match);

async fn bf_rmatch<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    do_re_match(bf_args, true)
}
bf_declare!(rmatch, bf_rmatch);

fn substitute(template: &str, subs: &[(isize, isize)], source: &str) -> Result<String, Error> {
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
        if start < 0 || start > end || end > (source.len() as isize) {
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

async fn bf_substitute<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, Error> {
    if bf_args.args.len() != 2 {
        return Err(E_INVARG);
    }
    let (template, subs) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(template), Variant::List(subs)) => (template, subs),
        _ => return Err(E_TYPE),
    };

    // Subs is of form {<start>, <end>, <replacements>, <subject>}
    // "replacement" and subject are what we're interested in.
    if subs.len() != 4 {
        return Err(E_INVARG);
    }

    let (Variant::List(subs), Variant::Str(source)) = (subs[2].variant(), subs[3].variant()) else {
        return Err(E_INVARG);
    };

    // Turn psubs into a Vec<(isize, isize)>. Raising errors on the way if they're not
    let mut mysubs = Vec::new();
    for sub in &subs[..] {
        let Variant::List(sub) = sub.variant() else {
            return Err(E_INVARG);
        };
        if sub.len() != 2 {
            return Err(E_INVARG);
        }
        let (Variant::Int(start), Variant::Int(end)) = (sub[0].variant(), sub[1].variant()) else {
            return Err(E_INVARG);
        };
        mysubs.push((*start as isize, *end as isize));
    }

    match substitute(template.as_str(), &mysubs, source.as_str()) {
        Ok(r) => Ok(Ret(v_string(r))),
        Err(e) => Err(e),
    }
}
bf_declare!(substitute, bf_substitute);

impl VM {
    pub(crate) fn register_bf_list_sets(&mut self) {
        self.builtins[offset_for_builtin("is_member")] = Arc::new(BfIsMember {});
        self.builtins[offset_for_builtin("listinsert")] = Arc::new(BfListinsert {});
        self.builtins[offset_for_builtin("listappend")] = Arc::new(BfListappend {});
        self.builtins[offset_for_builtin("listdelete")] = Arc::new(BfListdelete {});
        self.builtins[offset_for_builtin("listset")] = Arc::new(BfListset {});
        self.builtins[offset_for_builtin("setadd")] = Arc::new(BfSetadd {});
        self.builtins[offset_for_builtin("setremove")] = Arc::new(BfSetremove {});
        self.builtins[offset_for_builtin("match")] = Arc::new(BfMatch {});
        self.builtins[offset_for_builtin("rmatch")] = Arc::new(BfRmatch {});
        self.builtins[offset_for_builtin("substitute")] = Arc::new(BfSubstitute {});
    }
}

#[cfg(test)]
mod tests {
    use crate::builtins::bf_list_sets::{perform_regex_match, substitute};

    #[test]
    fn test_match_substitute() {
        let source = "*** Welcome to LambdaMOO!!!";
        let (overall, subs) = perform_regex_match("%(%w*%) to %(%w*%)", source, false, false)
            .unwrap()
            .unwrap();
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
    fn test_substitute_regression() {
        let source = "help @options";
        let (_, subs) = perform_regex_match("^help %('%|[^ <][^ ]*%)$", source, false, false)
            .unwrap()
            .unwrap();
        let result = substitute("%1", &subs, source).unwrap();
        assert_eq!(result, "@options");
    }

    #[test]
    fn test_substitute_off_by_one() {
        let source = "@edit-o";
        let (overall, subs) = perform_regex_match(
            "^@%([^-]*%)%(o%|opt?i?o?n?s?%|-o?p?t?i?o?n?s?%)$",
            source,
            false,
            false,
        )
        .unwrap()
        .unwrap();
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

    #[test]
    fn test_match_regression() {
        let source = "2";
        // In MOO this should yield (1,1). In Python re it's (0,1).
        // 'twas returning None because + support got broken.
        let (overall, _) = perform_regex_match("[0-9]+ *", source, false, false)
            .unwrap()
            .unwrap();
        assert_eq!(overall, (1, 1));
    }

    #[test]
    fn test_rmatch() {
        let m = perform_regex_match("o*b", "foobar", false, true)
            .unwrap()
            .unwrap();
        // {4, 4, {{0, -1}
        assert_eq!(
            m,
            (
                (4, 4),
                vec![
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1),
                    (0, -1)
                ]
            )
        );
    }
}
