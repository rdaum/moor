// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

use std::{
    io::{self, Write},
    ops::Range,
};

use ariadne::{CharSet, Config, Label, Report, ReportKind, Source, sources};
use itertools::Itertools;
use moor_var::{Symbol, Var, v_int, v_list, v_map, v_str, v_sym};

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
    // Handle ParseError specially with full span information
    if let CompileError::ParseError {
        message, details, ..
    } = error
    {
        let Some(src) = source else {
            eprintln!("Parse error: {}", message);
            return;
        };

        let Some((start, end)) = details.span else {
            eprintln!("Parse error: {}", message);
            return;
        };

        let source_id = source_name.to_string();
        let report = Report::build(ReportKind::Error, (source_id.clone(), start..end))
            .with_config(
                Config::default()
                    .with_color(use_color)
                    .with_char_set(CharSet::Unicode),
            )
            .with_message(message)
            .with_label(
                Label::new((source_id.clone(), start..end)).with_message("parser stopped here"),
            )
            .finish();

        let mut stderr = io::stderr().lock();
        let _ = report.write(sources([(source_id, src)]), &mut stderr);
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
        return;
    }

    // For all other compile errors, show file/line/column info with source context
    let context = error.context();
    let (line, col) = context.line_col;

    // If we have source, create a simple single-point report
    if let Some(src) = source {
        // Convert line/col to byte offset
        let mut current_line = 1;
        let mut current_col = 1;
        let mut offset = 0;

        for (idx, ch) in src.char_indices() {
            if current_line == line && current_col == col {
                offset = idx;
                break;
            }
            if ch == '\n' {
                current_line += 1;
                current_col = 1;
            } else {
                current_col += 1;
            }
        }

        let source_id = source_name.to_string();
        let report = Report::build(ReportKind::Error, (source_id.clone(), offset..offset + 1))
            .with_config(
                Config::default()
                    .with_color(use_color)
                    .with_char_set(CharSet::Unicode),
            )
            .with_message(error.to_string())
            .with_label(Label::new((source_id.clone(), offset..offset + 1)))
            .finish();

        let mut stderr = io::stderr().lock();
        let _ = report.write(sources([(source_id, src)]), &mut stderr);
        let _ = stderr.flush();
    } else {
        // No source available, just show file:line:col and message
        eprintln!("{}:{}:{}: {}", source_name, line, col, error);
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
    let mut builder = Report::build(ReportKind::Error, span.clone())
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
