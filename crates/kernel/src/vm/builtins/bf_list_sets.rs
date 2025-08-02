// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use ahash::HashMap;
use lazy_static::lazy_static;
use moor_compiler::offset_for_builtin;
use moor_var::{Associative, E_ARGS, E_INVARG, E_RANGE, E_TYPE, Error, Variant, FAILED_MATCH};
use moor_var::{
    IndexMode, List, Sequence, Var, VarType, v_empty_list, v_int, v_list, v_list_iter, v_map,
    v_obj, v_str, v_string,
};
use moor_common::matching::{ComplexMatchResult, complex_match_strings, complex_match_objects_keys};
use onig::{Region, SearchOptions, SyntaxBehavior, SyntaxOperator};
use std::ops::BitOr;
use std::sync::Mutex;

use crate::vm::builtins::BfRet::Ret;
use crate::vm::builtins::{BfCallState, BfErr, BfRet, BuiltinFunction};

fn bf_is_member(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (value, container) = (&bf_args.args[0], &bf_args.args[1]);
    // `is_member` is overloaded to work on both maps and lists, so `bf_list_sets.rs`
    // is not *really* a correct place for it, but `bf_list_sets_and_maps_too_i_guess.rs` is a bit silly.
    match container.variant() {
        Variant::List(list) => {
            if list
                .index_in(value, true)
                .map_err(BfErr::ErrValue)?
                .is_some()
            {
                Ok(Ret(v_int(1)))
            } else {
                Ok(Ret(v_int(0)))
            }
        }
        Variant::Map(map) => Ok(Ret(v_int(
            map.iter()
                .position(|(_item_key, item_value)| value.eq_case_sensitive(&item_value))
                .map(|pos| pos + 1)
                .unwrap_or(0) as i64,
        ))),
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn bf_listinsert(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let value = &bf_args.args[1];
    let list = &bf_args.args[0];
    if list.type_code() != VarType::TYPE_LIST {
        return Err(BfErr::Code(E_TYPE));
    }
    // If two args, treat as push. If three, treat as insert.
    if bf_args.args.len() == 2 {
        return Ok(Ret(list.push(value).map_err(BfErr::ErrValue)?));
    }
    let index = &bf_args.args[2];
    let res = list.insert(index, value, IndexMode::OneBased);
    Ok(Ret(res.map_err(BfErr::ErrValue)?))
}

fn bf_listappend(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let value = &bf_args.args[1];
    let list = &bf_args.args[0];
    if list.type_code() != VarType::TYPE_LIST {
        return Err(BfErr::Code(E_TYPE));
    }
    // If two args, treat as push. If three, treat as insert.
    if bf_args.args.len() == 2 {
        return Ok(Ret(list.push(value).map_err(BfErr::ErrValue)?));
    }
    let index = &bf_args.args[2];
    let res = list.insert(index, value, IndexMode::ZeroBased);
    Ok(Ret(res.map_err(BfErr::ErrValue)?))
}

fn bf_listdelete(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let index = bf_args.args[1].clone();
    let list = &bf_args.args[0];
    Ok(Ret(list
        .remove_at(&index, IndexMode::OneBased)
        .map_err(BfErr::ErrValue)?))
}

fn bf_listset(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let index = bf_args.args[2].clone();
    let value = bf_args.args[1].clone();
    let list = bf_args.args[0].clone();
    if list.type_code() != VarType::TYPE_LIST {
        return Err(BfErr::Code(E_TYPE));
    }
    Ok(Ret(list
        .index_set(&index, &value, IndexMode::OneBased)
        .map_err(BfErr::ErrValue)?))
}

fn bf_setadd(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let value = bf_args.args[1].clone();
    let list = bf_args.args[0].clone();
    let Some(list) = list.as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    Ok(Ret(list.set_add(&value).map_err(BfErr::ErrValue)?))
}

fn bf_setremove(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let value = bf_args.args[1].clone();
    let Some(list) = bf_args.args[0].as_list() else {
        return Err(BfErr::Code(E_TYPE));
    };
    Ok(Ret(list.set_remove(&value).map_err(BfErr::ErrValue)?))
}

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
            let escape = c_iter.next()?;
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
            s.push(c);
            let next = c_iter.next()?;
            c = next;
            if c == '^' || c == ']' {
                s.push(c);
                c = c_iter.next()?;
            }
            while c != ']' {
                s.push(c);
                c = c_iter.next()?;
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

lazy_static! {
    static ref MOO_REGEX_CACHE: Mutex<HashMap<(String, bool), Result<onig::Regex, onig::Error>>> =
        Default::default();
}

/// Perform regex match using LambdaMOO's "legacy" regular expression support, which is based on
/// pre-POSIX regexes.
/// To do this, we use oniguruma, which is a modern regex library that supports these old-style
/// regexes and a pile of other stuff.
fn perform_regex_match(
    pattern: &str,
    subject: &str,
    case_matters: bool,
    reverse: bool,
) -> Result<Option<MatchSpans>, Error> {
    let Some(translated_pattern) = translate_pattern(pattern) else {
        return Err(E_INVARG.msg("Invalid regex pattern"));
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
    syntax.set_behavior(SyntaxBehavior::SYNTAX_BEHAVIOR_ALLOW_DOUBLE_RANGE_OP_IN_CC);

    let mut cache_lock = MOO_REGEX_CACHE.lock().unwrap();
    let regex = cache_lock
        .entry((translated_pattern.clone(), case_matters))
        .or_insert_with(|| {
            onig::Regex::with_options(translated_pattern.as_str(), options, &syntax)
        });
    let regex = match regex {
        Ok(regex) => regex,
        Err(_) => {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        }
    };
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
fn do_re_match(bf_args: &mut BfCallState<'_>, reverse: bool) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let case_matters = if bf_args.args.len() == 3 {
        let Some(case_matters) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        case_matters == 1
    } else {
        false
    };

    // TODO: Regex pattern cache?
    let Some((overall, match_vec)) =
        perform_regex_match(pattern.as_str(), subject.as_str(), case_matters, reverse)
            .map_err(BfErr::ErrValue)?
    else {
        return Ok(Ret(v_empty_list()));
    };

    let subs = v_list_iter(
        match_vec
            .iter()
            .map(|(start, end)| v_list(&[v_int(*start as i64), v_int(*end as i64)])),
    );
    Ok(Ret(v_list(&[
        v_int(overall.0 as i64),
        v_int(overall.1 as i64),
        subs,
        bf_args.args[0].clone(),
    ])))
}
fn bf_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    do_re_match(bf_args, false)
}

fn bf_rmatch(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    do_re_match(bf_args, true)
}

lazy_static! {
    static ref PCRE_PATTERN_CACHE: Mutex<HashMap<(String, bool), Result<onig::Regex, onig::Error>>> =
        Default::default();
}

/// Perform a PCRE match using oniguruma.
/// If `map_support` is true, the return value is a list of maps, where each map contains the
/// matched text and the start and end positions of the match.
/// If `map_support` is false, the return value is a list of assoc-lists, where each assoc-list
/// contains the matched text and the start and end positions of the match.
/// If `case_matters` is true, the match is case-sensitive.
/// If `repeat` is true, the match is repeated until no more matches are found.
fn perform_pcre_match(
    map_support: bool,
    case_matters: bool,
    re: &str,
    target: &str,
    repeat: bool,
) -> Result<List, Error> {
    let case_insensitive = !case_matters;
    let cache_key = (re.to_string(), case_insensitive);
    let mut cache_lock = PCRE_PATTERN_CACHE.lock().unwrap();
    let regex = cache_lock.entry(cache_key).or_insert_with(|| {
        let options = if !case_insensitive {
            onig::RegexOptions::REGEX_OPTION_NONE
        } else {
            onig::RegexOptions::REGEX_OPTION_IGNORECASE
        };

        let syntax = onig::Syntax::perl();
        onig::Regex::with_options(re, options, syntax)
    });
    let regex = match regex {
        Ok(regex) => regex,
        Err(_) => {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        }
    };

    let mut region = Region::new();
    let mut matches = Vec::new();
    let mut start = 0;
    let end = target.len();
    while regex
        .search_with_options(
            target,
            start,
            end,
            SearchOptions::SEARCH_OPTION_NONE,
            Some(&mut region),
        )
        .is_some()
    {
        if map_support {
            let mut map = vec![];
            for i in 0..region.len() {
                let (start, end) = region.pos(i).unwrap();
                let match_map = vec![
                    (v_str("match"), v_str(&target[start..end])),
                    (
                        v_str("position"),
                        v_list(&[v_int((start as i64) + 1), v_int(end as i64)]),
                    ),
                ];
                map.push((v_string(i.to_string()), v_map(&match_map)));
            }
            let map = v_map(&map);
            matches.push(map);
            start = region.pos(0).unwrap().1;
        } else {
            let mut assoc_list = vec![];
            for i in 0..region.len() {
                let (start, end) = region.pos(i).unwrap();
                let match_list = vec![
                    v_list(&[v_str("match"), v_str(&target[start..end])]),
                    v_list(&[
                        v_str("position"),
                        v_list(&[v_int((start as i64) + 1), v_int(end as i64)]),
                    ]),
                ];
                assoc_list.push(v_list(&[v_string(i.to_string()), v_list(&match_list)]));
            }
            matches.push(v_list(&assoc_list));
            start = region.pos(0).unwrap().1;
        }
        if !repeat {
            break;
        }
    }

    Ok(List::mk_list(&matches))
}

fn perform_pcre_replace(target: &str, replace_str: &str) -> Result<String, Error> {
    let separator = {
        let mut chars = replace_str.chars();
        // First character must be 's'
        let Some(first_char) = chars.next() else {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        };
        if first_char != 's' {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        }

        // Next character is separator and must be either '/' or '!' and determines what the separator
        // is for the rest of time.
        let Some(sep_char) = chars.next() else {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        };

        if sep_char != '/' && sep_char != '!' {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        }

        sep_char
    };

    // Split using the separator
    let components: Vec<_> = replace_str.splitn(4, separator).collect();
    if components.len() < 2 {
        return Err(E_INVARG.msg("Invalid regex pattern"));
    };

    let (pattern, replacement) = (components[1], components[2]);

    let (global, case_insensitive) = if components.len() == 4 {
        (components[3].contains("g"), components[3].contains("i"))
    } else {
        (false, false)
    };

    let cache_key = (pattern.to_string(), case_insensitive);
    let mut cache_lock = PCRE_PATTERN_CACHE.lock().unwrap();
    let regex = cache_lock.entry(cache_key).or_insert_with(|| {
        let options = if !case_insensitive {
            onig::RegexOptions::REGEX_OPTION_NONE
        } else {
            onig::RegexOptions::REGEX_OPTION_IGNORECASE
        };

        let syntax = onig::Syntax::perl();
        onig::Regex::with_options(pattern, options, syntax)
    });
    let regex = match regex {
        Ok(regex) => regex,
        Err(_) => {
            return Err(E_INVARG.msg("Invalid regex pattern"));
        }
    };
    // If `global` we will replace all matches. Otherwise, just stop after the first
    let mut start = 0;
    let mut region = Region::new();
    let end = target.len();
    let mut matches = vec![];
    'outer: loop {
        let match_num = regex.search_with_options(
            target,
            start,
            end,
            SearchOptions::SEARCH_OPTION_NONE,
            Some(&mut region),
        );

        if match_num.is_none() {
            break;
        }
        if region.is_empty() {
            break;
        }
        for (match_start, end) in region.iter() {
            // Append the match to our matches.
            // If not `global`, break afterwords.
            // If global, move "start" past it, and continue
            matches.push((match_start, end));
            if !global {
                break 'outer;
            }
            start = end;
        }
    }

    // Now compose the string looking at the matches, replacing the `replacement` in every place
    let mut result = String::new();
    let mut offset = 0;

    // Iterate through all matches and compose the result string
    for (start, end) in matches {
        // Append the portion of the target string before the match
        result.push_str(&target[offset..start]);

        // Append the replacement string
        result.push_str(replacement);

        // Update the offset to the end of the current match
        offset = end;
    }

    // Append the remainder of the target string after the last match
    result.push_str(&target[offset..]);

    Ok(result)
}

fn bf_pcre_replace(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    // pcre_substitute(target, replace_str)
    // Given a replace_str like 's/frob/dog', and a target like "pet the frobs"
    // Should get 'pet the dogs'
    // If suffixed with 'g' or 'i', "global" and "case insensitive" applied respectively.
    // Separator is either '/' or '!' and is determined by looking at the first character after 's'
    // only 's' is supported.
    // (this abomination is only here for toast back compatibility.)
    if bf_args.args.len() != 2 {
        return Err(BfErr::ErrValue(
            E_ARGS.msg("pcre_replace() requires two arguments"),
        ));
    }

    let (Variant::Str(target), Variant::Str(replace_str)) =
        (bf_args.args[0].variant(), bf_args.args[1].variant())
    else {
        return Err(BfErr::ErrValue(
            E_TYPE.msg("pcre_replace() requires two string arguments"),
        ));
    };
    let (target, replace_str) = (target.as_str(), replace_str.as_str());

    match perform_pcre_replace(target, replace_str) {
        Ok(result) => Ok(Ret(v_str(&result))),
        Err(err) => Err(BfErr::ErrValue(err)),
    }
}
/*
From Toast:

Function: pcre_match

pcre_match -- The function pcre_match() searches subject for pattern using the Perl Compatible Regular Expressions library.

LIST pcre_match(STR subject, STR pattern [, ?case matters=0] [, ?repeat until no matches=1])

The return value is a list of maps containing each match. Each returned map will have a key which corresponds to either a named capture group or
 the number of the capture group being matched. The full match is always found in the key "0". The value of each key will be another map
  containing the keys 'match' and 'position'. Match corresponds to the text that was matched and position will return the indices of the substring within subject.

 In Moor, if maps features is disabled, the return is assoc-lists, which are lists of lists of two elements, the first being the key and the second being the value.

 => {["0" -> ["match" -> "09/12/1999", "position" -> {1, 10}], "1" -> ["match" -> "09", "position" -> {1, 2}], "2" -> ["match" -> "12", "position" -> {4, 5}], "3" -> ["match" -> "1999", "position" -> {7, 10}]], ["0" -> ["match" -> "01/21/1952", "position" -> {30, 39}], "1" -> ["match" -> "01", "position" -> {30, 31}], "2" -> ["match" -> "21", "position" -> {33, 34}], "3" -> ["match" -> "1952", "position" -> {36, 39}]]}
 */
fn bf_pcre_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let case_matters = if bf_args.args.len() >= 3 {
        let Some(case_matters) = bf_args.args[2].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        case_matters == 1
    } else {
        false
    };

    let repeat = if bf_args.args.len() == 4 {
        let Some(repeat) = bf_args.args[3].as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        repeat == 1
    } else {
        true
    };

    let map_support = bf_args.config.map_type;
    let result = match perform_pcre_match(
        map_support,
        case_matters,
        pattern.as_str(),
        subject.as_str(),
        repeat,
    ) {
        Ok(result) => result,
        Err(err) => return Err(BfErr::ErrValue(err)),
    };
    Ok(Ret(Var::from_variant(Variant::List(result))))
}

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
            return Err(E_INVARG.msg("Invalid number"));
        };

        // If the number is out of range, we'll raise an E_INVARG. E_RANGE would be nice, but
        // that's not what MOO does.
        if number > subs.len() {
            return Err(E_INVARG.msg("Number out of range"));
        }

        // Special case for 0
        let (start, end) = if number == 0 {
            (subs[0].0, subs[0].1)
        } else {
            // We're 1-indexed, so we'll subtract 1 from the number.
            let number = number - 1;

            // Look it up in matching `subs` pairs.
            (subs[number].0, subs[number].1)
        };

        // Now validate the range in the source string, and if the range is invalid, we just skip,
        // as this seems to be how LambdaMOO behaves.
        if start < 0 || start > end || end > (source.len() as isize) {
            continue;
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

fn bf_substitute(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (template, subs) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(template), Variant::List(subs)) => (template, subs),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    // Subs is of form {<start>, <end>, <replacements>, <subject>}
    // "replacement" and subject are what we're interested in.
    if subs.len() != 4 {
        return Err(BfErr::Code(E_INVARG));
    }

    let (Ok(a), Ok(b)) = (subs.index(2), subs.index(3)) else {
        return Err(BfErr::Code(E_INVARG));
    };
    let (Variant::List(subs), Variant::Str(source)) = (a.variant(), b.variant()) else {
        return Err(BfErr::Code(E_INVARG));
    };

    // Turn psubs into a Vec<(isize, isize)>. Raising errors on the way if they're not
    let mut mysubs = Vec::new();
    for sub in subs.iter() {
        let Some(sub) = sub.as_list() else {
            return Err(BfErr::Code(E_INVARG));
        };
        if sub.len() != 2 {
            return Err(BfErr::Code(E_INVARG));
        }
        let (Ok(start), Ok(end)) = (sub.index(0), sub.index(1)) else {
            return Err(BfErr::Code(E_INVARG));
        };
        let (Some(start), Some(end)) = (start.as_integer(), end.as_integer()) else {
            return Err(BfErr::Code(E_INVARG));
        };
        mysubs.push((start as isize, end as isize));
    }

    match substitute(template.as_str(), &mysubs, source.as_str()) {
        Ok(r) => Ok(Ret(v_string(r))),
        Err(e) => Err(BfErr::ErrValue(e)),
    }
}

/// ```moo
/// list slice(list|map alist [, list|str index [, any default_value]])
/// ```
/// Returns a list containing elements from `alist` based on the `index` parameter:
///
/// - If `alist` is a list of lists and `index` is an integer, returns a list containing
///   the element at the specified position from each sublist in `alist`.
/// - If `alist` is a list of lists and `index` is a list of integers, returns a list containing
///   lists of elements at the specified positions from each sublist in `alist`.
///
/// - If `alist` is a list of maps and `index` is a string, returns a list containing
///   the values associated with key `index` from each map in `alist`.
///   If `default_value` is provided, it will be used for any maps that don't contain the key.
/// ```moo
///   slice({{1,2,3},{4,5,6}}, 2) => {2, 5}
///   slice({{1,2,3},{4,5,6}}, {1, 3}) => {{1, 3}, {4, 6}}
///   slice({{"z", 1}, {"y", 2}, {"x",5}}, 2) => {1, 2, 5}.
///   slice({{"z", 1, 3}, {"y", 2, 4}}, {2, 1}) => {{1, "z"}, {2, "y"}}
///   slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6]}, "a") => {1, 5}
/// ```
fn bf_slice(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    // Get the collection (list or map)
    let collection = &bf_args.args[0];

    // Index must be provided
    let index = if bf_args.args.len() >= 2 {
        &bf_args.args[1]
    } else {
        return Err(BfErr::Code(E_ARGS));
    };

    // Optional default value for map lookups
    let default_value = if bf_args.args.len() == 3 {
        Some(&bf_args.args[2])
    } else {
        None
    };

    match collection.variant() {
        Variant::List(list) => {
            // Ensure we have a list of lists or maps
            if list.is_empty() {
                return Ok(Ret(v_empty_list()));
            }

            let first_item = list.index(0).map_err(BfErr::ErrValue)?;
            if !matches!(first_item.variant(), Variant::List(_) | Variant::Map(_)) {
                return Err(BfErr::Code(E_TYPE));
            }

            match index.variant() {
                // Case 1: List of lists + Integer index
                // This handles: slice({{1,2,3},{4,5,6}}, 2) => {2, 5}
                // For each sublist in the input list, extract the element at position 'idx'
                // and return a list of these elements
                Variant::Int(idx) => {
                    let idx = *idx as usize;
                    let mut result = Vec::with_capacity(list.len());

                    for item in list.iter() {
                        let Some(sublist) = item.as_list() else {
                            return Err(BfErr::Code(E_TYPE));
                        };

                        if idx < 1 || idx > sublist.len() {
                            return Err(BfErr::Code(E_RANGE));
                        }
                        // MOO is 1-indexed, so subtract 1
                        result.push(sublist.index(idx - 1).map_err(BfErr::ErrValue)?);
                    }

                    Ok(Ret(v_list(&result)))
                }

                // Case 2: List + List of indices
                // This handles: slice({{1,2,3},{4,5,6}}, {1, 3}) => {{1, 3}, {4, 6}}
                Variant::List(indices) => {
                    let mut result = Vec::with_capacity(list.len());

                    // Check if this is a list of lists
                    let first_item = list.index(0).map_err(BfErr::ErrValue)?;
                    if first_item.as_list().is_some() {
                        // This is a list of lists, extract elements from each sublist based on indices
                        // For each sublist in the input list, create a new list containing
                        // the elements at the positions specified in 'indices'
                        for item in list.iter() {
                            let Some(sublist) = item.as_list() else {
                                return Err(BfErr::Code(E_TYPE));
                            };

                            let mut subresult = Vec::with_capacity(indices.len());

                            for idx_var in indices.iter() {
                                let Some(idx) = idx_var.as_integer() else {
                                    return Err(BfErr::Code(E_TYPE));
                                };

                                let idx = idx as usize;
                                if idx < 1 || idx > sublist.len() {
                                    return Err(BfErr::Code(E_RANGE));
                                }
                                // MOO is 1-indexed, so subtract 1
                                subresult.push(sublist.index(idx - 1).map_err(BfErr::ErrValue)?);
                            }

                            result.push(v_list(&subresult));
                        }
                    }

                    Ok(Ret(v_list(&result)))
                }

                // Case 3: List of maps + String key
                // This handles: slice({["x" -> 1, "y" -> 2], ["x" -> 3, "z" -> 4]}, "x") => {1, 3}
                // For each map in the input list, extract the value associated with the key 'key'
                // and return a list of these values
                Variant::Str(key) => {
                    let mut result = Vec::with_capacity(list.len());

                    for item in list.iter() {
                        let Some(map) = item.as_map() else {
                            return Err(BfErr::Code(E_TYPE));
                        };

                        // Create a key Var from the string
                        let key_var = v_str(key.as_str());

                        // Try to get the value for this key
                        match map.get(&key_var) {
                            Ok(value) => result.push(value),
                            Err(_) => {
                                // Use default value if provided, otherwise error
                                // This handles: slice({#[["x", 1]], #[["z", 4]]}, "x", 0) => {1, 0}
                                if let Some(default) = default_value {
                                    result.push(default.clone());
                                } else {
                                    return Err(BfErr::Code(E_RANGE));
                                }
                            }
                        }
                    }

                    Ok(Ret(v_list(&result)))
                }

                _ => Err(BfErr::Code(E_TYPE)),
            }
        }

        // If the collection is a map, we don't support this yet
        Variant::Map(_) => Err(BfErr::Code(E_TYPE)),

        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn bf_complex_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }
    
    let token = match bf_args.args[0].variant() {
        Variant::Str(token) => token.as_str(),
        _ => return Err(BfErr::Code(E_TYPE)),
    };
    
    if bf_args.args.len() == 2 {
        // Two argument form: complex_match(token, strings)
        let strings = match bf_args.args[1].variant() {
            Variant::List(strings) => strings,
            _ => return Err(BfErr::Code(E_TYPE)),
        };
        
        let string_vars: Vec<Var> = strings.iter().collect();
        match complex_match_strings(token, &string_vars) {
            ComplexMatchResult::NoMatch => Ok(Ret(v_obj(FAILED_MATCH))),
            ComplexMatchResult::Single(result) => Ok(Ret(result)),
            ComplexMatchResult::Multiple(results) => {
                // For 2-arg form, return first match when multiple
                if !results.is_empty() {
                    Ok(Ret(results[0].clone()))
                } else {
                    Ok(Ret(v_obj(FAILED_MATCH)))
                }
            }
        }
    } else {
        // Three argument form: complex_match(token, objs, keys)
        let (objs, keys) = match (bf_args.args[1].variant(), bf_args.args[2].variant()) {
            (Variant::List(objs), Variant::List(keys)) => (objs, keys),
            _ => return Err(BfErr::Code(E_TYPE)),
        };
        
        let obj_vars: Vec<Var> = objs.iter().collect();
        let key_vars: Vec<Var> = keys.iter().collect();
        
        match complex_match_objects_keys(token, &obj_vars, &key_vars) {
            ComplexMatchResult::NoMatch => Ok(Ret(v_obj(FAILED_MATCH))),
            ComplexMatchResult::Single(result) => Ok(Ret(result)),
            ComplexMatchResult::Multiple(results) => {
                // For 3-arg form, return first match when multiple  
                if !results.is_empty() {
                    Ok(Ret(results[0].clone()))
                } else {
                    Ok(Ret(v_obj(FAILED_MATCH)))
                }
            }
        }
    }
}

pub(crate) fn register_bf_list_sets(builtins: &mut [Box<BuiltinFunction>]) {
    builtins[offset_for_builtin("is_member")] = Box::new(bf_is_member);
    builtins[offset_for_builtin("listinsert")] = Box::new(bf_listinsert);
    builtins[offset_for_builtin("listappend")] = Box::new(bf_listappend);
    builtins[offset_for_builtin("listdelete")] = Box::new(bf_listdelete);
    builtins[offset_for_builtin("listset")] = Box::new(bf_listset);
    builtins[offset_for_builtin("setadd")] = Box::new(bf_setadd);
    builtins[offset_for_builtin("setremove")] = Box::new(bf_setremove);
    builtins[offset_for_builtin("match")] = Box::new(bf_match);
    builtins[offset_for_builtin("rmatch")] = Box::new(bf_rmatch);
    builtins[offset_for_builtin("substitute")] = Box::new(bf_substitute);
    builtins[offset_for_builtin("pcre_match")] = Box::new(bf_pcre_match);
    builtins[offset_for_builtin("pcre_replace")] = Box::new(bf_pcre_replace);
    builtins[offset_for_builtin("slice")] = Box::new(bf_slice);
    builtins[offset_for_builtin("complex_match")] = Box::new(bf_complex_match);
}

#[cfg(test)]
mod tests {
    use crate::vm::builtins::bf_list_sets::{
        perform_pcre_match, perform_pcre_replace, perform_regex_match, substitute,
    };
    use moor_compiler::to_literal;
    use moor_var::{Var, Variant, v_int, v_list, v_map, v_str};

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

    /// This pattern was causing an E_INVARG in BfMatch, due to the "-" after the 9.
    /// Turning on SyntaxBehavior::SYNTAX_BEHAVIOR_ALLOW_DOUBLE_RANGE_OP_IN_CC seems to fix it.
    #[test]
    fn test_bug() {
        let problematic_regex = "^[]a-zA-Z0-9-%~`!@#$^&()=+{}[|';?/><.,]+$";
        perform_regex_match(problematic_regex, "foo", false, false).unwrap();
    }

    #[test]
    fn test_pcre_match() {
        // Example from toast manual:
        //  pcre_match("09/12/1999 other random text 01/21/1952", "([0-9]{2})/([0-9]{2})/([0-9]{4})")
        //  => {["0" -> ["match" -> "09/12/1999", "position" -> {1, 10}], "1" -> ["match" -> "09", "position" -> {1, 2}], "2" -> ["match" -> "12", "position" -> {4, 5}], "3" -> ["match" -> "1999", "position" -> {7, 10}]], ["0" -> ["match" -> "01/21/1952", "position" -> {30, 39}], "1" -> ["match" -> "01", "position" -> {30, 31}], "2" -> ["match" -> "21", "position" -> {33, 34}], "3" -> ["match" -> "1952", "position" -> {36, 39}]]}
        let regex = "([0-9]{2})/([0-9]{2})/([0-9]{4})";
        let target = "09/12/1999 other random text 01/21/1952";
        let result = perform_pcre_match(true, false, regex, target, false).unwrap();
        let v = Var::from_variant(Variant::List(result));
        let expected = v_list(&[v_map(&[
            (
                v_str("0"),
                v_map(&[
                    (v_str("match"), v_str("09/12/1999")),
                    (v_str("position"), v_list(&[v_int(1), v_int(10)])),
                ]),
            ),
            (
                v_str("1"),
                v_map(&[
                    (v_str("match"), v_str("09")),
                    (v_str("position"), v_list(&[v_int(1), v_int(2)])),
                ]),
            ),
            (
                v_str("2"),
                v_map(&[
                    (v_str("match"), v_str("12")),
                    (v_str("position"), v_list(&[v_int(4), v_int(5)])),
                ]),
            ),
            (
                v_str("3"),
                v_map(&[
                    (v_str("match"), v_str("1999")),
                    (v_str("position"), v_list(&[v_int(7), v_int(10)])),
                ]),
            ),
        ])]);
        assert_eq!(
            v,
            expected,
            "Expected: \n{}\nGot: \n{}",
            to_literal(&expected),
            to_literal(&v)
        );
    }

    #[test]
    fn test_pcre_replace() {
        assert_eq!(
            perform_pcre_replace("cats and dogs", "s/cat/dog").unwrap(),
            "dogs and dogs"
        );
        assert_eq!(
            perform_pcre_replace("cats and dogs", "s/moose/dog").unwrap(),
            "cats and dogs"
        );
        assert_eq!(
            perform_pcre_replace("cats and dogs", "s/\\w+/moose").unwrap(),
            "moose and dogs"
        );
        assert_eq!(
            perform_pcre_replace("cats and dogs", r#"s/\w+/moose/g"#).unwrap(),
            "moose moose moose"
        );
        assert_eq!(
            perform_pcre_replace("Cats and Dogs and cats", r#"s/cats/moose/ig"#).unwrap(),
            "moose and Dogs and moose"
        );
    }
}
