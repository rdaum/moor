// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! Builtin functions for list manipulation, set operations, and regular expression matching.

use ahash::HashMap;
use lazy_static::lazy_static;
use moor_common::matching::{
    ComplexMatchResult, complex_match_objects_keys_all,
    complex_match_objects_keys_with_fuzzy_threshold, complex_match_strings_all,
    complex_match_strings_with_fuzzy_threshold,
};
use moor_compiler::offset_for_builtin;
use moor_var::{
    Associative, E_ARGS, E_INVARG, E_MAXREC, E_RANGE, E_TYPE, Error, FAILED_MATCH, IndexMode, List,
    Sequence, Var, VarType, Variant, v_empty_list, v_int, v_list, v_list_iter, v_map, v_obj, v_str,
    v_string,
};
use onig::{MatchParam, Region, SearchOptions, SyntaxBehavior, SyntaxOperator};
use std::{
    ops::BitOr,
    sync::{Arc, Mutex},
};

use crate::{
    task_context::with_current_transaction,
    vm::builtins::{BfCallState, BfErr, BfRet, BfRet::Ret, BuiltinFunction},
};

/// Usage: `int is_member(any value, list|map|flyweight container)`
/// Returns the 1-based index of value in container if found, or 0 if not found.
/// Unlike the `in` operator, this function performs case-sensitive string comparison.
fn bf_is_member(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (value, container) = (&bf_args.args[0], &bf_args.args[1]);
    // `is_member` is overloaded to work on maps, lists, and flyweights, so `bf_list_sets.rs`
    // is not *really* a correct place for it, but `bf_list_sets_and_maps_and_flyweights_i_guess.rs` is a bit silly.
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
        Variant::Flyweight(flyweight) => {
            if flyweight
                .contents()
                .index_in(value, true)
                .map_err(BfErr::ErrValue)?
                .is_some()
            {
                Ok(Ret(v_int(1)))
            } else {
                Ok(Ret(v_int(0)))
            }
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

/// Usage: `list all_members(any value, list alist)`
/// Returns a list of all 1-based indices where value appears in alist.
/// Uses case-sensitive comparison for strings.
fn bf_all_members(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 2 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (value, list) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let indices: Vec<Var> = list
        .iter()
        .enumerate()
        .filter(|(_, item)| value.eq_case_sensitive(item))
        .map(|(i, _)| v_int((i + 1) as i64)) // 1-based indexing
        .collect();

    Ok(Ret(v_list(&indices)))
}

fn get_sequence_element(item: &Var, idx: usize) -> Result<Var, BfErr> {
    match item.variant() {
        Variant::List(list) => {
            if idx == 0 || idx > list.len() {
                return Err(BfErr::Code(E_RANGE));
            }
            Ok(list.index(idx - 1).map_err(BfErr::ErrValue)?)
        }
        Variant::Str(s) => {
            let Some(ch) = s.as_str().chars().nth(idx - 1) else {
                return Err(BfErr::Code(E_RANGE));
            };
            Ok(v_string(ch.to_string()))
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

fn parse_index_list(indices: &List) -> Result<Vec<usize>, BfErr> {
    if indices.is_empty() {
        return Err(BfErr::Code(E_RANGE));
    }
    let mut result = Vec::with_capacity(indices.len());
    for idx in indices.iter() {
        let Some(pos) = idx.as_integer() else {
            return Err(BfErr::Code(E_TYPE));
        };
        if pos <= 0 {
            return Err(BfErr::Code(E_RANGE));
        }
        result.push(pos as usize);
    }
    Ok(result)
}

/// Usage: `bool all(any value [, ...])`
/// Returns true if every argument is truthy. Returns true when no arguments are provided.
fn bf_all(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    for arg in bf_args.args.iter() {
        if !arg.is_true() {
            return Ok(Ret(bf_args.v_bool(false)));
        }
    }
    Ok(Ret(bf_args.v_bool(true)))
}

/// Usage: `bool none(list values)`
/// Returns true if no argument is truthy. Returns true when no arguments are provided.
fn bf_none(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    for arg in bf_args.args.iter() {
        if arg.is_true() {
            return Ok(Ret(bf_args.v_bool(false)));
        }
    }
    Ok(Ret(bf_args.v_bool(true)))
}

/// Usage: `list listinsert(list list, any value [, int index])`
/// Returns a copy of list with value inserted before the element at the given index.
/// If index is not provided, inserts at the beginning. Raises E_RANGE if index is invalid.
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

/// Usage: `list listappend(list list, any value [, int index])`
/// Returns a copy of list with value inserted after the element at the given index.
/// If index is not provided, appends at the end. Raises E_RANGE if index is invalid.
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

/// Usage: `list listdelete(list list, int index)`
/// Returns a copy of list with the element at index removed. Raises E_RANGE if index is
/// not in the range [1..length(list)].
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

/// Usage: `list listset(list list, any value, int index)`
/// Returns a copy of list with the element at index replaced by value. Raises E_RANGE
/// if index is not in [1..length(list)]. Prefer indexed assignment (list[i] = v) instead.
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

/// Usage: `list setadd(list set, any value)`
/// Returns a copy of list with value added at the end, but only if value is not already
/// present. Treats list as a mathematical set.
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

/// Usage: `list setremove(list set, any value)`
/// Returns a copy of list with the first occurrence of value removed. If value is not
/// present, returns an identical list.
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

fn byte_offset_to_char_index(s: &str, byte: usize) -> usize {
    s[..byte].chars().count()
}

fn char_range_to_byte_range(s: &str, start: isize, end: isize) -> Option<(usize, usize)> {
    if start < 1 || end < start {
        return None;
    }
    let mut start_byte = None;
    let mut end_byte = None;
    for (i, (byte_index, ch)) in s.char_indices().enumerate() {
        let pos = (i + 1) as isize;
        if pos == start {
            start_byte = Some(byte_index);
        }
        if pos == end {
            end_byte = Some(byte_index + ch.len_utf8());
            break;
        }
    }
    match (start_byte, end_byte) {
        (Some(start_byte), Some(end_byte)) => Some((start_byte, end_byte)),
        _ => None,
    }
}

type RegexCacheKey = (String, bool);
type RegexCacheValue = Result<Arc<onig::Regex>, onig::Error>;
type RegexCache = Mutex<HashMap<RegexCacheKey, RegexCacheValue>>;

lazy_static! {
    static ref MOO_REGEX_CACHE: RegexCache = Default::default();
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

    let regex = {
        let mut cache_lock = MOO_REGEX_CACHE.lock().unwrap();
        let regex = cache_lock
            .entry((translated_pattern.clone(), case_matters))
            .or_insert_with(|| {
                onig::Regex::with_options(translated_pattern.as_str(), options, &syntax)
                    .map(Arc::new)
            });
        match regex {
            Ok(regex) => Arc::clone(regex),
            Err(_) => {
                return Err(E_INVARG.msg("Invalid regex pattern"));
            }
        }
    };
    let (search_start, search_end) = if reverse {
        (subject.len(), 0)
    } else {
        (0, subject.len())
    };
    let mut region = Region::new();
    let search_result = match regex.search_with_param(
        subject,
        search_start,
        search_end,
        SearchOptions::SEARCH_OPTION_NONE,
        Some(&mut region),
        MatchParam::default(),
    ) {
        Ok(result) => result,
        Err(err) => {
            return Err(E_MAXREC.msg(format!("Regex search error: {}", err.description())));
        }
    };
    let Some(_) = search_result else {
        return Ok(None);
    };
    // Overall span
    let Some((start, end)) = region.pos(0) else {
        return Ok(None);
    };

    let overall = (
        (byte_offset_to_char_index(subject, start) + 1) as isize,
        byte_offset_to_char_index(subject, end) as isize,
    );
    // Now we'll iterate through the captures, and build up a Vec<Span> of the captured groups.
    // MOO match() returns 9 subpatterns, no more, no less. So we start with a Vec of 9
    // (-1, -1) pairs and then fill that in with the captured groups, if any.
    let mut match_vec = vec![(0, -1); 9];
    for i in 1..=8 {
        if let Some((start, end)) = region.pos(i) {
            match_vec[i - 1] = (
                (byte_offset_to_char_index(subject, start) + 1) as isize,
                byte_offset_to_char_index(subject, end) as isize,
            );
        }
    }

    Ok(Some((overall, match_vec)))
}

/// Common code for both match and rmatch functions.
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
/// Usage: `list match(str subject, str pattern [, bool case_matters])`
/// Searches for the first occurrence of pattern in subject using MOO regular expressions.
/// Returns {} if no match, or {start, end, replacements, subject} where replacements is
/// a list of 9 {start,end} pairs for captured subpatterns. By default case-insensitive.
fn bf_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    do_re_match(bf_args, false)
}

/// Usage: `list rmatch(str subject, str pattern [, bool case_matters])`
/// Like match(), but searches for the last occurrence of pattern in subject.
fn bf_rmatch(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    do_re_match(bf_args, true)
}

lazy_static! {
    static ref PCRE_PATTERN_CACHE: RegexCache = Default::default();
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
    let regex = {
        let mut cache_lock = PCRE_PATTERN_CACHE.lock().unwrap();
        let regex = cache_lock.entry(cache_key).or_insert_with(|| {
            let options = if !case_insensitive {
                onig::RegexOptions::REGEX_OPTION_NONE
            } else {
                onig::RegexOptions::REGEX_OPTION_IGNORECASE
            };

            let syntax = onig::Syntax::perl();
            onig::Regex::with_options(re, options, syntax).map(Arc::new)
        });
        match regex {
            Ok(regex) => Arc::clone(regex),
            Err(_) => {
                return Err(E_INVARG.msg("Invalid regex pattern"));
            }
        }
    };

    let mut region = Region::new();
    let mut matches = Vec::new();
    let mut start = 0;
    let end = target.len();
    while match regex.search_with_param(
        target,
        start,
        end,
        SearchOptions::SEARCH_OPTION_NONE,
        Some(&mut region),
        MatchParam::default(),
    ) {
        Ok(result) => result,
        Err(err) => {
            return Err(E_MAXREC.msg(format!("Regex search error: {}", err.description())));
        }
    }
    .is_some()
    {
        let capture = |index| {
            let (start, end) = region.pos(index)?;
            let matched = target.get(start..end)?;
            Some((matched, start, end))
        };
        let mut max_index = None;
        for i in 0..region.len() {
            if region.pos(i).is_some() {
                max_index = Some(i);
            }
        }
        let Some(max_index) = max_index else {
            break;
        };

        if map_support {
            let mut map = vec![];
            for i in 0..=max_index {
                let (match_value, start_char, end_char) =
                    if let Some((matched, start, end)) = capture(i) {
                        (
                            v_str(matched),
                            (byte_offset_to_char_index(target, start) + 1) as i64,
                            byte_offset_to_char_index(target, end) as i64,
                        )
                    } else {
                        (v_str(""), 0, -1)
                    };
                let match_map = vec![
                    (v_str("match"), match_value),
                    (
                        v_str("position"),
                        v_list(&[v_int(start_char), v_int(end_char)]),
                    ),
                ];
                map.push((v_string(i.to_string()), v_map(&match_map)));
            }
            let map = v_map(&map);
            matches.push(map);
        } else {
            let mut assoc_list = vec![];
            for i in 0..=max_index {
                let (match_value, start_char, end_char) =
                    if let Some((matched, start, end)) = capture(i) {
                        (
                            v_str(matched),
                            (byte_offset_to_char_index(target, start) + 1) as i64,
                            byte_offset_to_char_index(target, end) as i64,
                        )
                    } else {
                        (v_str(""), 0, -1)
                    };
                let match_list = vec![
                    v_list(&[v_str("match"), match_value]),
                    v_list(&[
                        v_str("position"),
                        v_list(&[v_int(start_char), v_int(end_char)]),
                    ]),
                ];
                assoc_list.push(v_list(&[v_string(i.to_string()), v_list(&match_list)]));
            }
            matches.push(v_list(&assoc_list));
        }
        let Some((_, _, end)) = capture(0) else {
            break;
        };
        start = end;
        if !repeat {
            break;
        }
    }

    Ok(List::mk_list(&matches))
}

/// Substitutes capture group references ($0, $1, $2, etc.) in a replacement string with
/// the corresponding captured text from the regex match.
/// - $0 refers to the entire match
/// - $1, $2, ... $9 refer to capture groups 1-9
/// - $$ produces a literal $ character
fn apply_capture_groups(replacement: &str, target: &str, captures: &[(usize, usize)]) -> String {
    let mut result = String::new();
    let mut chars = replacement.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '$' {
            result.push(c);
            continue;
        }

        // We've seen a $, check what follows
        match chars.peek() {
            Some('$') => {
                // $$ -> literal $
                result.push('$');
                chars.next();
            }
            Some(d) if d.is_ascii_digit() => {
                // $N -> capture group N
                let digit = chars.next().unwrap();
                let group_num = digit.to_digit(10).unwrap() as usize;

                if group_num < captures.len() {
                    let (start, end) = captures[group_num];
                    if start <= end && end <= target.len() {
                        result.push_str(&target[start..end]);
                    }
                }
                // If group doesn't exist, we just skip it (no output)
            }
            _ => {
                // $ followed by non-digit, non-$ -> literal $
                result.push('$');
            }
        }
    }

    result
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
    // Need at least 3 components: "s", pattern, and replacement
    if components.len() < 3 {
        return Err(E_INVARG.msg("Invalid regex pattern"));
    };

    let (pattern, replacement) = (components[1], components[2]);

    let (global, case_insensitive) = if components.len() == 4 {
        (components[3].contains("g"), components[3].contains("i"))
    } else {
        (false, false)
    };

    let cache_key = (pattern.to_string(), case_insensitive);
    let regex = {
        let mut cache_lock = PCRE_PATTERN_CACHE.lock().unwrap();
        let regex = cache_lock.entry(cache_key).or_insert_with(|| {
            let options = if !case_insensitive {
                onig::RegexOptions::REGEX_OPTION_NONE
            } else {
                onig::RegexOptions::REGEX_OPTION_IGNORECASE
            };

            let syntax = onig::Syntax::perl();
            onig::Regex::with_options(pattern, options, syntax).map(Arc::new)
        });
        match regex {
            Ok(regex) => Arc::clone(regex),
            Err(_) => {
                return Err(E_INVARG.msg("Invalid regex pattern"));
            }
        }
    };
    // If `global` we will replace all matches. Otherwise, just stop after the first
    let mut start = 0;
    let mut region = Region::new();
    let end = target.len();
    // Each match stores all capture groups: group 0 is the full match, groups 1+ are capture groups
    let mut matches: Vec<Vec<(usize, usize)>> = vec![];
    loop {
        let match_num = match regex.search_with_param(
            target,
            start,
            end,
            SearchOptions::SEARCH_OPTION_NONE,
            Some(&mut region),
            MatchParam::default(),
        ) {
            Ok(result) => result,
            Err(err) => {
                return Err(E_MAXREC.msg(format!("Regex search error: {}", err.description())));
            }
        };

        if match_num.is_none() {
            break;
        }
        if region.is_empty() {
            break;
        }

        // Collect all capture groups for this match
        let mut captures = vec![];
        for i in 0..region.len() {
            if let Some((cap_start, cap_end)) = region.pos(i) {
                captures.push((cap_start, cap_end));
            } else {
                // Non-participating group - push a sentinel value
                captures.push((0, 0));
            }
        }

        // Get the full match bounds (group 0)
        let Some((match_start, match_end)) = region.pos(0) else {
            break;
        };

        matches.push(captures);

        if !global {
            break;
        }

        // Move past this match for the next iteration
        // Handle zero-length matches by advancing at least one byte
        start = if match_end > match_start {
            match_end
        } else {
            match_end + 1
        };
        if start > end {
            break;
        }
    }

    // Now compose the string looking at the matches, replacing the `replacement` in every place
    let mut result = String::new();
    let mut offset = 0;

    // Iterate through all matches and compose the result string
    for captures in &matches {
        // Get the full match bounds (group 0)
        let (match_start, match_end) = captures[0];

        // Append the portion of the target string before the match
        result.push_str(&target[offset..match_start]);

        // Apply capture group substitution to the replacement string
        let substituted = apply_capture_groups(replacement, target, captures);
        result.push_str(&substituted);

        // Update the offset to the end of the current match
        offset = match_end;
    }

    // Append the remainder of the target string after the last match
    result.push_str(&target[offset..]);

    Ok(result)
}

/// Usage: `str pcre_replace(str target, str replace_str)`
/// Performs PCRE-style replacement using syntax like 's/pattern/replacement/flags'.
/// Supports 'g' (global) and 'i' (case-insensitive) flags.
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
/// Usage: `list pcre_match(str subject, str pattern [, bool case_matters] [, bool repeat])`
/// Searches subject for pattern using Perl Compatible Regular Expressions.
/// Returns a list of maps (or assoc-lists if maps disabled) containing each match.
/// Each map has keys for capture groups, with "0" being the full match.
/// Values contain 'match' (matched text) and 'position' (start/end indices).
fn bf_pcre_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }
    let (subject, pattern) = match (bf_args.args[0].variant(), bf_args.args[1].variant()) {
        (Variant::Str(subject), Variant::Str(pattern)) => (subject, pattern),
        _ => return Err(BfErr::Code(E_TYPE)),
    };

    let case_matters = bf_args.args.len() >= 3 && bf_args.args[2].is_true();
    let repeat = bf_args.args.len() != 4 || bf_args.args[3].is_true();

    let result = match perform_pcre_match(
        true,
        case_matters,
        pattern.as_str(),
        subject.as_str(),
        repeat,
    ) {
        Ok(result) => result,
        Err(err) => return Err(BfErr::ErrValue(err)),
    };
    Ok(Ret(Var::from_list(result)))
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
        let source_char_len = source.chars().count() as isize;
        if start < 1 || start > end || end > source_char_len {
            continue;
        }

        let Some((start_byte, end_byte)) = char_range_to_byte_range(source, start, end) else {
            continue;
        };
        // Now append the corresponding substring to `result`.
        result.push_str(&source[start_byte..end_byte]);
        if let Some(last_c) = last_c {
            result.push(last_c);
        }
    }
    Ok(result)
}

/// Usage: `str substitute(str template, list subs)`
/// Performs substitutions on template using match data from match() or rmatch().
/// In template, %0 is replaced by the full match, %1-%9 by captured subpatterns,
/// and %% by a literal %. Raises E_INVARG for invalid substitutions.
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
/// list slice(LIST alist [, INT | LIST | STR index [, ANY default_map_value]])
/// ```
/// Returns values collected from each element of `alist` according to `index`.
///
/// - `index` defaults to `1` (the first element/character).
/// - If `index` is an integer, the indexed element is pulled from each sublist/string.
/// - If `index` is a list of integers, each position is pulled and returned as a list.
/// - If `index` is a string, every element in `alist` must be a map; the keyed value is returned (or `default_map_value` if provided and the key is missing).
///
/// Examples:
/// ```moo
/// slice({{1,2,3},{4,5,6}}, 2)                     => {2, 5}
/// slice({{1,2,3},{4,5,6}}, {1, 3})               => {{1, 3}, {4, 6}}
/// slice({{"z", 1}, {"y", 2}, {"x",5}}, 2)          => {1, 2, 5}
/// slice({{"z", 1, 3}, {"y", 2, 4}}, {2, 1})       => {{1, "z"}, {2, "y"}}
/// slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6]}, "a") => {1, 5}
/// slice({["a" -> 1], ["z" -> 2]}, "b", 0)          => {0, 0}
/// ```
fn bf_slice(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 3 {
        return Err(BfErr::Code(E_ARGS));
    }

    // Get the collection (list)
    let collection = &bf_args.args[0];

    let default_index = v_int(1);
    let index = if bf_args.args.len() >= 2 {
        &bf_args.args[1]
    } else {
        &default_index
    };

    let default_value = if bf_args.args.len() >= 3 {
        Some(bf_args.args.index(2).map_err(BfErr::ErrValue)?)
    } else {
        None
    };

    let Variant::List(list) = collection.variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    if list.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    match index.variant() {
        Variant::Int(idx) => {
            if idx <= 0 {
                return Err(BfErr::Code(E_RANGE));
            }
            let mut result = Vec::with_capacity(list.len());
            for item in list.iter() {
                match item.variant() {
                    Variant::List(_) | Variant::Str(_) => {
                        result.push(get_sequence_element(&item, idx as usize)?);
                    }
                    _ => return Err(BfErr::Code(E_TYPE)),
                }
            }
            Ok(Ret(v_list(&result)))
        }
        Variant::List(indices) => {
            let parsed_indices = parse_index_list(indices)?;
            let first_item = list.index(0).map_err(BfErr::ErrValue)?;
            let is_string_sequence = matches!(first_item.variant(), Variant::Str(_));
            let is_list_sequence = matches!(first_item.variant(), Variant::List(_));

            if !is_string_sequence && !is_list_sequence {
                return Err(BfErr::Code(E_TYPE));
            }

            let mut result = Vec::with_capacity(list.len());
            for item in list.iter() {
                if is_string_sequence && !matches!(item.variant(), Variant::Str(_)) {
                    return Err(BfErr::Code(E_TYPE));
                }
                if is_list_sequence && !matches!(item.variant(), Variant::List(_)) {
                    return Err(BfErr::Code(E_TYPE));
                }

                let mut subresult = Vec::with_capacity(parsed_indices.len());
                for &idx in &parsed_indices {
                    subresult.push(get_sequence_element(&item, idx)?);
                }

                result.push(v_list(&subresult));
            }

            Ok(Ret(v_list(&result)))
        }
        Variant::Str(key) => {
            let mut result = Vec::with_capacity(list.len());
            for item in list.iter() {
                let Some(map) = item.as_map() else {
                    return Err(BfErr::Code(E_TYPE));
                };

                let key_var = v_str(key.as_str());
                if let Ok(value) = map.get(&key_var) {
                    result.push(value);
                } else if let Some(default) = default_value.clone() {
                    result.push(default);
                } else {
                    return Err(BfErr::Code(E_RANGE));
                }
            }
            Ok(Ret(v_list(&result)))
        }
        _ => Err(BfErr::Code(E_TYPE)),
    }
}

/// Helper function to handle ComplexMatchResult for simple string/var cases
fn handle_simple_match_result(result: ComplexMatchResult<Var>) -> Result<BfRet, BfErr> {
    match result {
        ComplexMatchResult::NoMatch => Ok(Ret(v_obj(FAILED_MATCH))),
        ComplexMatchResult::Single(result) => Ok(Ret(result)),
        ComplexMatchResult::Multiple(results) => {
            if !results.is_empty() {
                Ok(Ret(results[0].clone()))
            } else {
                Ok(Ret(v_obj(FAILED_MATCH)))
            }
        }
    }
}

/// Usage: `any complex_match(str token, list targets [, list keys] [, num fuzzy_threshold])`
/// Performs complex pattern matching with fuzzy matching support.
/// fuzzy_threshold: 0.0 = no fuzzy, 0.5 = reasonable default, 1.0 = very permissive
/// Also accepts boolean for backward compatibility (false = 0.0, true = 0.5)
fn bf_complex_match(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(token) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let token = token.as_str();

    // Parse arguments: token, targets, [keys], [fuzzy_threshold]
    // 2 args: complex_match(token, targets) - fuzzy_threshold=0.5, no keys
    // 3 args: complex_match(token, targets, keys) - fuzzy_threshold=0.5, keys provided (or fuzzy if not list)
    // 4 args: complex_match(token, targets, keys, fuzzy_threshold) - explicit fuzzy setting

    let use_keys = bf_args.args.len() >= 3 && matches!(bf_args.args[2].variant(), Variant::List(_));
    let fuzzy_threshold = if bf_args.args.len() >= 4 || (bf_args.args.len() == 3 && !use_keys) {
        let fuzzy_arg = if bf_args.args.len() >= 4 {
            &bf_args.args[3]
        } else {
            &bf_args.args[2] // 3-arg form where third arg is fuzzy, not keys
        };

        match fuzzy_arg.variant() {
            Variant::Float(f) => f,
            Variant::Int(i) => i as f64,
            _ => {
                // Backward compatibility: treat as boolean
                if fuzzy_arg.is_true() { 0.5 } else { 0.0 }
            }
        }
    } else {
        0.0 // Default: no fuzzy matching
    };

    // Three/four argument form with keys: complex_match(token, objs, keys, [fuzzy])
    if use_keys {
        let (Variant::List(objs), Variant::List(keys)) =
            (bf_args.args[1].variant(), bf_args.args[2].variant())
        else {
            return Err(BfErr::Code(E_TYPE));
        };

        let obj_vars: Vec<Var> = objs.iter().collect();
        let key_vars: Vec<Var> = keys.iter().collect();

        // Validate that keys and targets have the same length
        if obj_vars.len() != key_vars.len() {
            return Err(BfErr::Code(E_INVARG));
        }

        return handle_simple_match_result(complex_match_objects_keys_with_fuzzy_threshold(
            token,
            &obj_vars,
            &key_vars,
            fuzzy_threshold,
        ));
    }

    // Two argument form: complex_match(token, strings_or_objects)
    let Variant::List(candidates) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    let candidate_vars: Vec<Var> = candidates.iter().collect();

    // Check if we have objects - if so, extract names and match against those
    let has_objects = candidate_vars
        .iter()
        .any(|v| matches!(v.variant(), Variant::Obj(_)));

    if !has_objects {
        return handle_simple_match_result(complex_match_strings_with_fuzzy_threshold(
            token,
            &candidate_vars,
            fuzzy_threshold,
        ));
    }

    // Extract names from objects and match against those
    let mut object_names = Vec::new();
    let mut objects = Vec::new();

    for candidate in &candidate_vars {
        let Variant::Obj(obj) = candidate.variant() else {
            continue; // Skip non-objects in mixed lists
        };

        // Get the object's name using name_of
        let name_result = with_current_transaction(|world_state| {
            world_state.name_of(&bf_args.task_perms_who(), &obj)
        });
        let Ok(name_str) = name_result else {
            continue; // Skip objects without valid name
        };

        object_names.push(v_string(name_str));
        objects.push(candidate.clone());
    }

    match complex_match_strings_with_fuzzy_threshold(token, &object_names, fuzzy_threshold) {
        ComplexMatchResult::NoMatch => Ok(Ret(v_obj(FAILED_MATCH))),
        ComplexMatchResult::Single(result) => {
            // Find which object corresponds to the matched name and return the object
            for (i, name) in object_names.iter().enumerate() {
                if name == &result {
                    return Ok(Ret(objects[i].clone()));
                }
            }
            Ok(Ret(v_obj(FAILED_MATCH)))
        }
        ComplexMatchResult::Multiple(results) => {
            // For 2-arg form, return first matching object when multiple
            if results.is_empty() {
                return Ok(Ret(v_obj(FAILED_MATCH)));
            }

            for (i, name) in object_names.iter().enumerate() {
                if name == &results[0] {
                    return Ok(Ret(objects[i].clone()));
                }
            }
            Ok(Ret(v_obj(FAILED_MATCH)))
        }
    }
}

/// Usage: `list complex_matches(str token, list targets [, list keys] [, num fuzzy_threshold])`
/// Returns all matches from the best (highest priority) tier as a list.
/// If no matches are found, returns an empty list.
/// Keys can be a list of strings (one per target) or a list of lists of strings (multiple per target).
fn bf_complex_matches(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let Variant::Str(token) = bf_args.args[0].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };
    let token = token.as_str();

    let Variant::List(targets) = bf_args.args[1].variant() else {
        return Err(BfErr::Code(E_TYPE));
    };

    // Parse arguments: token, targets, [keys], [fuzzy_threshold]
    // 2 args: complex_matches(token, targets) - fuzzy_threshold=0.0, no keys
    // 3 args: complex_matches(token, targets, keys) OR complex_matches(token, targets, fuzzy)
    // 4 args: complex_matches(token, targets, keys, fuzzy_threshold)

    let use_keys = bf_args.args.len() >= 3 && matches!(bf_args.args[2].variant(), Variant::List(_));
    let fuzzy_threshold = if bf_args.args.len() >= 4 || (bf_args.args.len() == 3 && !use_keys) {
        let fuzzy_arg = if bf_args.args.len() >= 4 {
            &bf_args.args[3]
        } else {
            &bf_args.args[2] // 3-arg form where third arg is fuzzy, not keys
        };

        match fuzzy_arg.variant() {
            Variant::Float(f) => f,
            Variant::Int(i) => i as f64,
            _ => {
                // Backward compatibility: treat as boolean
                if fuzzy_arg.is_true() { 0.5 } else { 0.0 }
            }
        }
    } else {
        0.0 // Default: no fuzzy matching
    };

    // If keys are provided, use the keys-based matching
    if use_keys {
        let Variant::List(keys) = bf_args.args[2].variant() else {
            return Err(BfErr::Code(E_TYPE));
        };

        let target_vars: Vec<Var> = targets.iter().collect();
        let key_vars: Vec<Var> = keys.iter().collect();

        // Validate that keys and targets have the same length
        if target_vars.len() != key_vars.len() {
            return Err(BfErr::Code(E_INVARG));
        }

        let matches =
            complex_match_objects_keys_all(token, &target_vars, &key_vars, fuzzy_threshold);
        return Ok(Ret(v_list(&matches)));
    }

    // No keys - match against targets directly
    let candidate_vars: Vec<Var> = targets.iter().collect();
    let matches = complex_match_strings_all(token, &candidate_vars, fuzzy_threshold);

    Ok(Ret(v_list(&matches)))
}

/// Usage: `list sort(list values [, list keys] [, int natural] [, int reverse])`
/// Sorts a list of values, optionally using a parallel list of keys for sorting.
/// All elements must be the same type (int, float, obj, err, or str).
/// Strings are compared case-insensitively, optionally using natural sort order.
fn bf_sort(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.is_empty() || bf_args.args.len() > 4 {
        return Err(BfErr::Code(E_ARGS));
    }

    let values = &bf_args.args[0];
    let Variant::List(values_list) = values.variant() else {
        return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
            "First argument to sort() must be a list".to_string()
        })));
    };

    // Empty list case
    if values_list.is_empty() {
        return Ok(Ret(v_empty_list()));
    }

    // Determine if we're sorting by keys (arg 2) or by values
    let (sort_by, _keys_list) = if bf_args.args.len() >= 2 {
        let keys = &bf_args.args[1];
        let Variant::List(keys_list) = keys.variant() else {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                "Second argument to sort() must be a list".to_string()
            })));
        };

        // If keys list is non-empty, use it for sorting
        if !keys_list.is_empty() {
            // Validate that keys and values have same length
            if keys_list.len() != values_list.len() {
                return Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
                    format!(
                        "sort() keys list length ({}) must match values list length ({})",
                        keys_list.len(),
                        values_list.len()
                    )
                })));
            }
            (keys_list, Some(keys_list))
        } else {
            (values_list, None)
        }
    } else {
        (values_list, None)
    };

    // Parse natural sort flag (arg 3)
    let natural = bf_args.args.len() >= 3 && bf_args.args[2].is_true();

    // Parse reverse flag (arg 4)
    let reverse = bf_args.args.len() >= 4 && bf_args.args[3].is_true();

    // Validate all elements in sort_by are the same type
    let first_elem = sort_by.index(0).map_err(BfErr::ErrValue)?;
    let sort_type = first_elem.type_code();

    // Check if type is sortable
    match sort_type {
        VarType::TYPE_INT
        | VarType::TYPE_FLOAT
        | VarType::TYPE_OBJ
        | VarType::TYPE_ERR
        | VarType::TYPE_STR => {}
        _ => {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "sort() cannot sort lists containing {} values",
                    sort_type.to_literal()
                )
            })));
        }
    }

    // Validate all elements are the same type
    for (idx, elem) in sort_by.iter().enumerate() {
        if elem.type_code() != sort_type {
            return Err(BfErr::ErrValue(E_TYPE.with_msg(|| {
                format!(
                    "sort() requires all elements to be the same type, but element at index {} is {} while others are {}",
                    idx + 1,
                    elem.type_code().to_literal(),
                    sort_type.to_literal()
                )
            })));
        }
    }

    // Create index vector and sort it
    let mut indices: Vec<usize> = (0..values_list.len()).collect();

    // Sort indices based on the sort_by list elements
    indices.sort_by(|&a, &b| {
        let elem_a = sort_by.index(a).unwrap();
        let elem_b = sort_by.index(b).unwrap();

        let ordering = match (elem_a.variant(), elem_b.variant()) {
            (Variant::Int(a), Variant::Int(b)) => a.cmp(&b),
            (Variant::Float(a), Variant::Float(b)) => {
                // Handle NaN: NaN is considered equal to itself and less than any other value
                if a.is_nan() && b.is_nan() {
                    std::cmp::Ordering::Equal
                } else if a.is_nan() {
                    std::cmp::Ordering::Less
                } else if b.is_nan() {
                    std::cmp::Ordering::Greater
                } else {
                    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
                }
            }
            (Variant::Obj(a), Variant::Obj(b)) => a.cmp(&b),
            (Variant::Err(a), Variant::Err(b)) => a.name().cmp(&b.name()),
            (Variant::Str(a), Variant::Str(b)) => {
                if natural {
                    natord::compare_ignore_case(a.as_str(), b.as_str())
                } else {
                    // Case-insensitive comparison
                    a.as_str().to_lowercase().cmp(&b.as_str().to_lowercase())
                }
            }
            _ => unreachable!("Type validation should have caught this"),
        };

        if reverse {
            ordering.reverse()
        } else {
            ordering
        }
    });

    // Build result list using sorted indices from values_list
    let result: Vec<Var> = indices
        .iter()
        .map(|&idx| values_list.index(idx).unwrap())
        .collect();

    Ok(Ret(v_list(&result)))
}

/// Usage: `list|str reverse(list|str input)`
/// Returns a copy of the list with elements in reverse order, or a string with
/// characters reversed by Unicode code points.
fn bf_reverse(bf_args: &mut BfCallState<'_>) -> Result<BfRet, BfErr> {
    if bf_args.args.len() != 1 {
        return Err(BfErr::Code(E_ARGS));
    }

    let input = &bf_args.args[0];

    match input.variant() {
        Variant::List(list) => {
            // Reverse the list
            let mut reversed: Vec<Var> = list.iter().collect();
            reversed.reverse();
            Ok(Ret(v_list(&reversed)))
        }
        Variant::Str(s) => {
            // Reverse the string by Unicode scalar values (code points)
            let reversed: String = s.as_str().chars().rev().collect();
            Ok(Ret(v_str(&reversed)))
        }
        _ => Err(BfErr::ErrValue(E_INVARG.with_msg(|| {
            format!(
                "reverse() requires a list or string, got {}",
                input.type_code().to_literal()
            )
        }))),
    }
}

pub(crate) fn register_bf_list_sets(builtins: &mut [BuiltinFunction]) {
    builtins[offset_for_builtin("is_member")] = bf_is_member;
    builtins[offset_for_builtin("all_members")] = bf_all_members;
    builtins[offset_for_builtin("all")] = bf_all;
    builtins[offset_for_builtin("none")] = bf_none;
    builtins[offset_for_builtin("listinsert")] = bf_listinsert;
    builtins[offset_for_builtin("listappend")] = bf_listappend;
    builtins[offset_for_builtin("listdelete")] = bf_listdelete;
    builtins[offset_for_builtin("listset")] = bf_listset;
    builtins[offset_for_builtin("setadd")] = bf_setadd;
    builtins[offset_for_builtin("setremove")] = bf_setremove;
    builtins[offset_for_builtin("match")] = bf_match;
    builtins[offset_for_builtin("rmatch")] = bf_rmatch;
    builtins[offset_for_builtin("substitute")] = bf_substitute;
    builtins[offset_for_builtin("pcre_match")] = bf_pcre_match;
    builtins[offset_for_builtin("pcre_replace")] = bf_pcre_replace;
    builtins[offset_for_builtin("slice")] = bf_slice;
    builtins[offset_for_builtin("complex_match")] = bf_complex_match;
    builtins[offset_for_builtin("complex_matches")] = bf_complex_matches;
    builtins[offset_for_builtin("sort")] = bf_sort;
    builtins[offset_for_builtin("reverse")] = bf_reverse;
}

#[cfg(test)]
mod tests {
    use crate::vm::builtins::bf_list_sets::{
        perform_pcre_match, perform_pcre_replace, perform_regex_match, substitute,
    };
    use moor_compiler::to_literal;
    use moor_var::{E_MAXREC, Var, v_int, v_list, v_map, v_str};

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

    #[test]
    fn test_match_unicode_indices() {
        let source = "héllo";
        let (overall, subs) = perform_regex_match("%(é%)", source, false, false)
            .unwrap()
            .unwrap();
        assert_eq!(overall, (2, 2));
        assert_eq!(subs[0], (2, 2));
        let result = substitute("%1", &subs, source).unwrap();
        assert_eq!(result, "é");
    }

    #[test]
    fn test_rmatch_unicode_indices() {
        let source = "héllö";
        let (overall, _) = perform_regex_match("l", source, false, true)
            .unwrap()
            .unwrap();
        assert_eq!(overall, (4, 4));
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
        let v = Var::from_list(result);
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
    fn test_pcre_match_optional_group_unmatched() {
        let regex = "(a)?b";
        let target = "b";
        let result = perform_pcre_match(true, false, regex, target, false).unwrap();
        let v = Var::from_list(result);
        let expected = v_list(&[v_map(&[(
            v_str("0"),
            v_map(&[
                (v_str("match"), v_str("b")),
                (v_str("position"), v_list(&[v_int(1), v_int(1)])),
            ]),
        )])]);
        assert_eq!(
            v,
            expected,
            "Expected: \n{}\nGot: \n{}",
            to_literal(&expected),
            to_literal(&v)
        );
    }

    #[test]
    fn test_pcre_match_unicode_indices() {
        let regex = "(é)";
        let target = "héllo";
        let result = perform_pcre_match(true, false, regex, target, false).unwrap();
        let v = Var::from_list(result);
        let match_map = v_map(&[
            (v_str("match"), v_str("é")),
            (v_str("position"), v_list(&[v_int(2), v_int(2)])),
        ]);
        let expected = v_list(&[v_map(&[
            (v_str("0"), match_map.clone()),
            (v_str("1"), match_map),
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
    fn test_pcre_match_retry_limit() {
        let regex = "(a|b|ab)*bc";
        let target = "ababababababababababababababababababababababababababababacbc";
        let err = perform_pcre_match(false, false, regex, target, false).unwrap_err();
        assert_eq!(err.err_type, E_MAXREC);
        assert!(err.message().contains("retry-limit-in-match"));
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

    #[test]
    fn test_pcre_replace_capture_groups() {
        // Basic capture group substitution from issue #606
        assert_eq!(
            perform_pcre_replace("Foobar", "s/(bar)/t$1t/i").unwrap(),
            "Footbart"
        );

        // $0 refers to the full match
        assert_eq!(
            perform_pcre_replace("hello world", r#"s/\w+/[$0]/g"#).unwrap(),
            "[hello] [world]"
        );

        // Multiple capture groups
        assert_eq!(
            perform_pcre_replace("John Smith", r#"s/(\w+) (\w+)/$2, $1/"#).unwrap(),
            "Smith, John"
        );

        // Capture groups with global flag
        assert_eq!(
            perform_pcre_replace("cat bat rat", r#"s/(\w)at/$1ot/g"#).unwrap(),
            "cot bot rot"
        );

        // Case insensitive with capture groups
        assert_eq!(
            perform_pcre_replace("CAT cat Cat", r#"s/(c)(at)/$1-$2/ig"#).unwrap(),
            "C-AT c-at C-at"
        );

        // $$ for literal dollar sign
        assert_eq!(
            perform_pcre_replace("price 100", r#"s/(\d+)/$$$$1/"#).unwrap(),
            "price $$1"
        );

        // Non-existing capture group is ignored
        assert_eq!(
            perform_pcre_replace("hello", r#"s/(hello)/$1 $2/"#).unwrap(),
            "hello "
        );

        // Complex example from the issue (singularization rule pattern)
        assert_eq!(
            perform_pcre_replace(
                "agenda",
                r#"s/(agend|addend|millenni|dat|extrem|bacteri|desiderat|strat|candelabr|errat|ov|symposi|curricul|quor)a$/$1um/i"#
            )
            .unwrap(),
            "agendum"
        );
    }

    #[test]
    fn test_pcre_replace_bad_tokenization() {
        // Issue #605: Bad tokenization should return E_INVARG, not panic
        // Missing replacement part
        assert!(perform_pcre_replace("hello", "s/hello").is_err());

        // Only separator
        assert!(perform_pcre_replace("hello", "s/").is_err());

        // Empty string after s
        assert!(perform_pcre_replace("hello", "s").is_err());

        // Just 's'
        assert!(perform_pcre_replace("hello", "s").is_err());

        // Wrong first character
        assert!(perform_pcre_replace("hello", "r/foo/bar/").is_err());

        // Wrong separator character
        assert!(perform_pcre_replace("hello", "s#foo#bar#").is_err());
    }

    #[test]
    fn test_pcre_replace_unicode() {
        // Basic UTF-8 replacement
        assert_eq!(
            perform_pcre_replace("héllo wörld", "s/wörld/world/").unwrap(),
            "héllo world"
        );

        // Capture group with multi-byte characters
        assert_eq!(
            perform_pcre_replace("日本語テスト", "s/(日本語)/[$1]/").unwrap(),
            "[日本語]テスト"
        );

        // Multiple capture groups with mixed ASCII and Unicode
        assert_eq!(
            perform_pcre_replace("café au lait", r#"s/(café) (au) (lait)/$3 $2 $1/"#).unwrap(),
            "lait au café"
        );

        // Global replacement with Unicode
        assert_eq!(
            perform_pcre_replace("🐱 and 🐱 and 🐱", r#"s/(🐱)/[$1]/g"#).unwrap(),
            "[🐱] and [🐱] and [🐱]"
        );

        // Unicode in replacement string
        assert_eq!(
            perform_pcre_replace("hello", "s/hello/привет/").unwrap(),
            "привет"
        );
    }
}
