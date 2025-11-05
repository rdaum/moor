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

//! Helpers to transform Pest parser errors into user-facing diagnostics.

use std::{
    borrow::Cow,
    cmp::min,
    io::{self, Write},
    ops::Range,
};

use ariadne::{CharSet, Config, Label, Report, ReportKind, Source};
use itertools::Itertools;
use moor_var::{Symbol, Var, v_int, v_list, v_map, v_str, v_sym};
use pest::error::{Error, ErrorVariant, InputLocation};

use crate::parse::moo::Rule;
use moor_common::model::{CompileContext, CompileError, ParseErrorDetails};

/// Verbosity levels for rendering diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticVerbosity {
    /// Single-line summary only.
    Summary,
    /// Summary with source context showing error location.
    SourceContext,
    /// Source context plus textual notes (expected tokens, hints).
    Detailed,
}

/// Rendering options for compiler diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticRenderOptions {
    pub verbosity: DiagnosticVerbosity,
    pub use_graphics: bool,
    pub use_color: bool,
}

impl Default for DiagnosticRenderOptions {
    fn default() -> Self {
        Self {
            verbosity: DiagnosticVerbosity::Summary,
            use_graphics: false,
            use_color: false,
        }
    }
}

/// Emit a compile error directly to stderr with rich formatting.
///
/// This function writes error diagnostics directly to stderr using Ariadne's rendering,
/// preserving ANSI color codes and graphical output. For parse errors with source text,
/// it will display the error location with context. For other errors, it falls back to
/// simple text output.
pub fn emit_compile_error(
    error: &CompileError,
    source: Option<&str>,
    source_name: &str,
    use_color: bool,
) {
    let CompileError::ParseError {
        message, details, ..
    } = error
    else {
        eprintln!("Compile error: {}", error);
        return;
    };

    let Some(src) = source else {
        eprintln!("Parse error: {}", message);
        return;
    };

    let Some((start, end)) = details.span else {
        eprintln!("Parse error: {}", message);
        return;
    };

    let report = Report::build(ReportKind::Error, source_name, start)
        .with_config(
            Config::default()
                .with_color(use_color)
                .with_char_set(CharSet::Unicode),
        )
        .with_message(message)
        .with_label(Label::new((source_name, start..end)).with_message("parser stopped here"))
        .finish();

    let mut stderr = io::stderr().lock();
    let _ = report.write((source_name, Source::from(src)), &mut stderr);
    let _ = stderr.flush();

    // Add helpful notes after the main report
    if details.expected_tokens.is_empty() && details.notes.is_empty() {
        return;
    }

    let _ = writeln!(&mut stderr);

    if !details.expected_tokens.is_empty() {
        let quoted: Vec<String> = details
            .expected_tokens
            .iter()
            .map(|token| format!("`{}`", token))
            .collect();

        if quoted.len() == 1 {
            let _ = writeln!(&mut stderr, "help: expected token {}", quoted[0]);
        } else {
            let _ = writeln!(
                &mut stderr,
                "help: expected one of {}",
                format_list(quoted.iter().map(|s| s.as_str()))
            );
        }
    }

    for note in &details.notes {
        let _ = writeln!(&mut stderr, "help: {}", note);
    }
}

/// Format a [`CompileError`] according to the requested diagnostic options.
///
/// For non-parse errors this falls back to the standard string form. For parse errors the caller
/// should pass the original source text so that detailed renderings can display accurate spans.
pub fn format_compile_error(
    error: &CompileError,
    source: Option<&str>,
    options: DiagnosticRenderOptions,
) -> Vec<String> {
    match error {
        CompileError::ParseError {
            error_position,
            context,
            end_line_col,
            message,
            details,
        } => format_parse_error(
            error_position,
            context,
            *end_line_col,
            message,
            details,
            source,
            options,
        ),
        _ => vec![error.to_string()],
    }
}

/// Produce a human-friendly summary string plus structured diagnostic details for a Pest error.
pub fn build_parse_error_details(
    program_text: &str,
    error: &Error<Rule>,
) -> (String, ParseErrorDetails) {
    let summary = summarize_error(error);
    let expected_tokens = extract_expected_tokens(error);
    let notes = collect_notes(program_text, error);

    let span_range = compute_span(program_text, error);
    let span = Some((span_range.start, span_range.end));

    let details = ParseErrorDetails {
        span,
        expected_tokens,
        notes,
    };

    (summary, details)
}

fn format_parse_error(
    error_position: &CompileContext,
    context_line: &str,
    end_line_col: Option<(usize, usize)>,
    summary: &str,
    details: &ParseErrorDetails,
    source: Option<&str>,
    options: DiagnosticRenderOptions,
) -> Vec<String> {
    let mut lines = vec![format!(
        "Failure to parse program @ {}/{}: {}",
        error_position.line_col.0, error_position.line_col.1, summary
    )];

    if options.verbosity == DiagnosticVerbosity::Summary {
        return lines;
    }

    // For SourceContext and Detailed, show the error location
    let use_graphical = options.use_graphics && source.is_some() && details.span.is_some();

    if use_graphical {
        let src = source.unwrap();
        let (start, end) = details.span.unwrap();
        let report = render_report(src, summary, start..end, options.use_color);
        lines.extend(report.lines().map(|line| line.to_string()));
    } else {
        lines.extend(render_plain_context(
            error_position,
            context_line,
            end_line_col,
        ));
    }

    // Only add notes for Detailed level
    if options.verbosity == DiagnosticVerbosity::Detailed {
        append_expected_tokens(&mut lines, details);
        append_notes(&mut lines, &details.notes);
    }

    lines
}

fn summarize_error(error: &Error<Rule>) -> String {
    let ErrorVariant::ParsingError {
        positives,
        negatives,
    } = &error.variant
    else {
        return error.variant.message().to_string();
    };

    let positive_descs: Vec<_> = positives.iter().map(describe_rule).collect();
    let negative_descs: Vec<_> = negatives.iter().map(describe_rule).collect();
    let expected = dedupe_descriptions(&positive_descs);
    let unexpected = dedupe_descriptions(&negative_descs);

    if expected.is_empty() && unexpected.is_empty() {
        return "unexpected parser failure".to_string();
    }

    let expected_str = if expected.is_empty() {
        None
    } else {
        Some(format_list(expected.iter().map(|d| d.label.as_ref())))
    };
    let unexpected_str = if unexpected.is_empty() {
        None
    } else {
        Some(format_list(unexpected.iter().map(|d| d.label.as_ref())))
    };

    match (expected_str, unexpected_str) {
        (Some(exp), Some(unexp)) => format!("unexpected {unexp}; expected {exp}"),
        (Some(exp), None) => format!("expected {exp}"),
        (None, Some(unexp)) => format!("unexpected {unexp}"),
        (None, None) => "unexpected parser failure".to_string(),
    }
}

fn extract_expected_tokens(error: &Error<Rule>) -> Vec<String> {
    let tokens = error
        .parse_attempts()
        .map(|parse_attempts| {
            parse_attempts
                .expected_tokens()
                .into_iter()
                .map(|token| token.to_string())
                .filter(|token| {
                    // Filter out whitespace tokens - not helpful for users
                    !matches!(
                        token.as_str(),
                        " " | "  " | "   " | "    " | "\n" | "\r" | "\r\n" | "\t"
                    )
                })
                .map(|token| {
                    // Normalize all-caps keywords to lowercase, but keep type literals uppercase
                    let is_type_literal = matches!(
                        token.as_str(),
                        "STR" | "OBJ" | "NUM" | "INT" | "FLOAT" | "LIST" | "ERR"
                    );
                    if !is_type_literal && token.chars().all(|c| c.is_ascii_uppercase() || c == '_')
                    {
                        token.to_lowercase()
                    } else {
                        token
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    dedupe_strings(tokens)
}

fn collect_notes(program_text: &str, error: &Error<Rule>) -> Vec<String> {
    let mut notes = Vec::new();

    notes.extend(
        error
            .variant
            .positives()
            .iter()
            .flat_map(|rule| describe_rule(rule).hint)
            .map(|hint| hint.to_string()),
    );

    // When we fail at end-of-file, highlight that explicitly.
    if let InputLocation::Pos(pos) = error.location
        && pos >= program_text.len()
        && !program_text.ends_with('\n')
    {
        notes.push("file ends here; is a terminator missing?".to_string());
    }

    dedupe_strings(notes)
}

fn append_expected_tokens(lines: &mut Vec<String>, details: &ParseErrorDetails) {
    if details.expected_tokens.is_empty() {
        return;
    }
    let quoted: Vec<String> = details
        .expected_tokens
        .iter()
        .map(|token| format!("`{}`", token))
        .collect();
    let message = if quoted.len() == 1 {
        format!("help: expected token {}", quoted[0])
    } else {
        format!(
            "help: expected one of {}",
            format_list(quoted.iter().map(|s| s.as_str()))
        )
    };
    lines.push(message);
}

fn append_notes(lines: &mut Vec<String>, notes: &[String]) {
    for note in notes {
        lines.push(format!("help: {}", note));
    }
}

fn dedupe_strings(strings: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for s in strings {
        if !out.iter().any(|existing| existing == &s) {
            out.push(s);
        }
    }
    out
}

fn compute_span(program_text: &str, error: &Error<Rule>) -> Range<usize> {
    match error.location {
        InputLocation::Pos(pos) => {
            let end = min(program_text.len(), pos.saturating_add(1));
            pos..end
        }
        InputLocation::Span((start, end)) => {
            let clamped_start = min(start, program_text.len());
            let clamped_end = min(end, program_text.len().max(clamped_start));
            clamped_start..clamped_end
        }
    }
}

fn render_plain_context(
    error_position: &CompileContext,
    context_line: &str,
    _end_line_col: Option<(usize, usize)>,
) -> Vec<String> {
    let mut lines = Vec::new();
    let trimmed = context_line.trim_end_matches(['\r', '\n']);

    lines.push(format!(
        "   line {} column {}:",
        error_position.line_col.0, error_position.line_col.1
    ));

    // Insert an inline marker at the error position - works with proportional fonts
    let start_col = error_position.line_col.1.saturating_sub(1); // Convert to 0-indexed
    let marker = " ⚠ ";

    let marked_line = if start_col < trimmed.len() {
        format!(
            "{}{}{}",
            &trimmed[..start_col],
            marker,
            &trimmed[start_col..]
        )
    } else {
        format!("{}{}", trimmed, marker)
    };

    lines.push(format!("   {}", marked_line));
    lines
}

fn render_report(program_text: &str, summary: &str, span: Range<usize>, use_color: bool) -> String {
    let mut builder = Report::build(ReportKind::Error, (), span.start)
        .with_config(
            Config::default()
                .with_color(use_color)
                .with_char_set(CharSet::Unicode),
        )
        .with_message(summary.to_string());

    builder = builder.with_label(Label::new(span).with_message("parser stopped here"));

    let report = builder.finish();
    report.write_to_string(Source::from(program_text))
}

fn dedupe_descriptions(descriptions: &[RuleDescriptor]) -> Vec<RuleDescriptor> {
    let mut out = Vec::new();
    for desc in descriptions {
        if !out
            .iter()
            .any(|existing: &RuleDescriptor| existing.group == desc.group)
        {
            out.push(desc.clone());
        }
    }
    out
}

fn describe_rule(rule: &Rule) -> RuleDescriptor {
    use Rule::*;

    match rule {
        add | sub | mul | div | modulus | pow | land | lor | eq | neq | lt | gt | lte | gte
        | in_range | bitand | bitor | bitxor | bitshl | bitlshr | bitshr => RuleDescriptor {
            label: Cow::Borrowed("an operator"),
            hint: Some(Cow::Borrowed(
                "Use operators like +, -, *, /, %, &&, ||, ==, !=, <, >, etc.",
            )),
            group: Cow::Borrowed("binary_operator"),
        },
        assign => RuleDescriptor {
            label: Cow::Borrowed("an assignment"),
            hint: Some(Cow::Borrowed(
                "Assignments use = with a variable name on the left, e.g. foo = expr.",
            )),
            group: Cow::Borrowed("assignment"),
        },
        index_range => RuleDescriptor {
            label: Cow::Borrowed("a range index like [1..5]"),
            hint: Some(Cow::Borrowed(
                "Range indexing uses [start..end] to get a slice.",
            )),
            group: Cow::Borrowed("index_range"),
        },
        index_single => RuleDescriptor {
            label: Cow::Borrowed("an index like [1]"),
            hint: Some(Cow::Borrowed(
                "Single indexing uses [position] to get one element.",
            )),
            group: Cow::Borrowed("index_single"),
        },
        verb_call | verb_expr_call => RuleDescriptor {
            label: Cow::Borrowed("a verb call like :verb(args)"),
            hint: Some(Cow::Borrowed(
                "Verb calls use :verb_name(...) or :expr(...).",
            )),
            group: Cow::Borrowed("verb_call"),
        },
        prop | prop_expr => RuleDescriptor {
            label: Cow::Borrowed("a property access like .name"),
            hint: None,
            group: Cow::Borrowed("property_access"),
        },
        cond_expr => RuleDescriptor {
            label: Cow::Borrowed("a conditional"),
            hint: Some(Cow::Borrowed(
                "Conditional expressions use `? then_expr | else_expr`.",
            )),
            group: Cow::Borrowed("conditional_expr"),
        },
        arglist => RuleDescriptor {
            label: Cow::Borrowed("an argument list `(…)`"),
            hint: Some(Cow::Borrowed(
                "Function calls require parentheses around arguments.",
            )),
            group: Cow::Borrowed("argument_list"),
        },
        integer => RuleDescriptor {
            label: Cow::Borrowed("an integer literal"),
            hint: None,
            group: Cow::Borrowed("integer_literal"),
        },
        float => RuleDescriptor {
            label: Cow::Borrowed("a floating-point literal"),
            hint: None,
            group: Cow::Borrowed("float_literal"),
        },
        string => RuleDescriptor {
            label: Cow::Borrowed("a string literal"),
            hint: Some(Cow::Borrowed(
                "Strings must be surrounded by double quotes.",
            )),
            group: Cow::Borrowed("string_literal"),
        },
        list => RuleDescriptor {
            label: Cow::Borrowed("a list literal like {1, 2, 3}"),
            hint: Some(Cow::Borrowed("List literals go inside braces `{}`.")),
            group: Cow::Borrowed("list_literal"),
        },
        map => RuleDescriptor {
            label: Cow::Borrowed("a map literal like [key -> value]"),
            hint: None,
            group: Cow::Borrowed("map_literal"),
        },
        lambda => RuleDescriptor {
            label: Cow::Borrowed("a lambda body `{params} => expr`"),
            hint: Some(Cow::Borrowed(
                "Lambda expressions use `{param, ...} => expression`.",
            )),
            group: Cow::Borrowed("lambda"),
        },
        fn_expr => RuleDescriptor {
            label: Cow::Borrowed("a function expression `fn (...) ... endfn`"),
            hint: Some(Cow::Borrowed(
                "Function expressions use `fn (params) ... endfn`.",
            )),
            group: Cow::Borrowed("fn_expr"),
        },
        builtin_call => RuleDescriptor {
            label: Cow::Borrowed("a builtin function call"),
            hint: None,
            group: Cow::Borrowed("builtin_call"),
        },
        ident => RuleDescriptor {
            label: Cow::Borrowed("an identifier"),
            hint: Some(Cow::Borrowed(
                "Identifiers start with a letter or underscore and may contain digits.",
            )),
            group: Cow::Borrowed("identifier"),
        },
        verb_decl => RuleDescriptor {
            label: Cow::Borrowed("a verb declaration"),
            hint: Some(Cow::Borrowed(
                "Verb declarations use: verb name (this none this) owner: #1 flags: \"rxd\"",
            )),
            group: Cow::Borrowed("verb_decl"),
        },
        prop_def => RuleDescriptor {
            label: Cow::Borrowed("a property definition"),
            hint: Some(Cow::Borrowed(
                "Property definitions use: property name (owner: #1, flags: \"\") = value;",
            )),
            group: Cow::Borrowed("prop_def"),
        },
        prop_set => RuleDescriptor {
            label: Cow::Borrowed("a property override"),
            hint: Some(Cow::Borrowed(
                "Property overrides use: override name = value; or override name (owner: #1, flags: \"\") = value;",
            )),
            group: Cow::Borrowed("prop_set"),
        },
        expr | primary => RuleDescriptor {
            label: Cow::Borrowed("an expression"),
            hint: None,
            group: Cow::Borrowed("expression"),
        },
        number | digits | digit_part | fraction | pos_exponent | neg_exponent | exponent_float
        | point_float => RuleDescriptor {
            label: Cow::Borrowed("a numeric literal"),
            hint: None,
            group: Cow::Borrowed("number"),
        },
        _ => {
            let name = format_rule_name(rule);
            RuleDescriptor {
                label: Cow::Owned(name.clone()),
                hint: None,
                group: Cow::Owned(name),
            }
        }
    }
}

fn format_rule_name(rule: &Rule) -> String {
    let debug = format!("{rule:?}");
    debug
        .split('_')
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_list<'a, I>(values: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let values = values.into_iter().collect_vec();
    match values.as_slice() {
        [] => String::new(),
        [first] => first.to_string(),
        [first, second] => format!("{first} or {second}"),
        _ => {
            let (last, rest) = values.split_last().unwrap();
            format!("{}, or {}", rest.join(", "), last)
        }
    }
}

#[derive(Clone)]
struct RuleDescriptor {
    label: Cow<'static, str>,
    hint: Option<Cow<'static, str>>,
    group: Cow<'static, str>,
}

trait ErrorVariantExt<R> {
    fn positives(&self) -> &[R];
}

impl<R> ErrorVariantExt<R> for ErrorVariant<R> {
    fn positives(&self) -> &[R] {
        match self {
            ErrorVariant::ParsingError { positives, .. } => positives,
            _ => &[],
        }
    }
}

/// Extension trait to write reports into strings without allocating intermediate buffers in
/// callers.
trait ReportWrite {
    fn write_to_string<C: ariadne::Cache<()>>(&self, cache: C) -> String;
}

impl ReportWrite for Report<'_, Range<usize>> {
    fn write_to_string<C: ariadne::Cache<()>>(&self, cache: C) -> String {
        let mut buffer = Vec::new();
        self.write(cache, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap_or_else(|_| String::new())
    }
}

/// Convert a [`CompileError`] into a MOO map structure with all diagnostic information.
///
/// Returns a map with keys:
///   - "type": "parse" or "other"
///   - "message": error summary string
///   - "line": line number (int)
///   - "column": column number (int)
///   - "context": the source line where error occurred (str)
///   - "source": full source code (str, optional)
///   - "span_start": start position in source (int, optional)
///   - "span_end": end position in source (int, optional)
///   - "expected_tokens": list of expected token strings
///   - "notes": list of hint strings
pub fn compile_error_to_map(error: &CompileError, source: Option<&str>, use_symbols: bool) -> Var {
    let sym_or_str = |s: &str| {
        if use_symbols {
            v_sym(Symbol::mk(s))
        } else {
            v_str(s)
        }
    };

    match error {
        CompileError::ParseError {
            error_position,
            context,
            message,
            details,
            ..
        } => {
            let expected_tokens_list = v_list(
                &details
                    .expected_tokens
                    .iter()
                    .map(|t| v_str(t))
                    .collect::<Vec<_>>(),
            );

            let notes_list = v_list(&details.notes.iter().map(|n| v_str(n)).collect::<Vec<_>>());

            let mut fields = vec![
                (sym_or_str("type"), sym_or_str("parse")),
                (sym_or_str("message"), v_str(message)),
                (sym_or_str("line"), v_int(error_position.line_col.0 as i64)),
                (
                    sym_or_str("column"),
                    v_int(error_position.line_col.1 as i64),
                ),
                (sym_or_str("context"), v_str(context)),
                (sym_or_str("expected_tokens"), expected_tokens_list),
                (sym_or_str("notes"), notes_list),
            ];

            // Include source and span if available
            if let Some(src) = source {
                fields.push((sym_or_str("source"), v_str(src)));
            }
            if let Some((start, end)) = details.span {
                fields.push((sym_or_str("span_start"), v_int(start as i64)));
                fields.push((sym_or_str("span_end"), v_int(end as i64)));
            }

            v_map(&fields)
        }
        _ => v_map(&[
            (sym_or_str("type"), sym_or_str("other")),
            (sym_or_str("message"), v_str(&error.to_string())),
        ]),
    }
}
