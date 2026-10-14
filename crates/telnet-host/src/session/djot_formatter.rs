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

//! Djot to ANSI terminal formatter using jotdown parser, colored output, and tabled.
//!
//! With [`Markup::Mxp`] the output is also MXP: `& < >` become entities, links become
//! `<SEND>` (commands) or `<A>` (http/https), and every line starts in secure mode
//! (`ESC [1z`). Escaping happens after layout, so table widths stay those of the visible text.

use super::moo_highlighter::highlight_moo;
use colored::{Color, ColoredString, Colorize};
use jotdown::{
    Alignment, Container, Event, ListKind, OrderedListNumbering, OrderedListStyle, Parser,
};
use tabled::{
    builder::Builder as TableBuilder,
    settings::{Alignment as TabledAlignment, Modify, Style, object::Columns},
};

/// Render djot markup to ANSI-colored terminal output.
/// If `utf8` is true, use Unicode characters for bullets, quotes, tables, etc.
/// If false, use ASCII-safe alternatives.
#[cfg(test)]
pub fn djot_to_terminal(input: &str, utf8: bool) -> String {
    djot_to_terminal_with_options(
        input,
        RenderOptions {
            utf8,
            ..RenderOptions::default()
        },
    )
}

/// The markup language of rendered output, besides ANSI styling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Markup {
    /// ANSI styling only.
    #[default]
    Ansi,
    /// ANSI styling inside MXP secure lines, with MXP links.
    Mxp,
}

/// Output options for telnet Djot rendering.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RenderOptions {
    pub(crate) utf8: bool,
    pub(crate) screen_reader_mode: bool,
    pub(crate) markup: Markup,
}

/// Render djot markup to terminal output using explicit render options.
pub(crate) fn djot_to_terminal_with_options(input: &str, options: RenderOptions) -> String {
    if options.markup == Markup::Ansi {
        let mut renderer = TerminalRenderer::new(options);
        renderer.render(Parser::new(input));
        return renderer.output;
    }
    // Markers in the input would be read as links; drop them.
    let input: String = input.chars().filter(|c| !is_link_marker(*c)).collect();
    let mut renderer = TerminalRenderer::new(options);
    renderer.render(Parser::new(&input));
    finish_mxp(&renderer.output)
}

/// MXP secure line mode: tags in the rest of the line are parsed.
const MXP_SECURE_LINE: &str = "\x1b[1z";

// Private-use markers the renderer leaves in its output around links; `finish_mxp` turns them
// into tags after layout. `<open> href <HREF_END> text <close>`.
const SEND_OPEN: char = '\u{E000}';
const A_OPEN: char = '\u{E001}';
const HREF_END: char = '\u{E002}';
const SEND_CLOSE: char = '\u{E003}';
const A_CLOSE: char = '\u{E004}';

fn is_link_marker(c: char) -> bool {
    ('\u{E000}'..='\u{E004}').contains(&c)
}

/// The MXP element for a link destination, and the `href` to give it.
fn mxp_link(dest: &str) -> Option<(char, char, String)> {
    let (scheme, target) = dest.split_once("://")?;
    let (open, close, href) = if scheme == "moo" {
        let command = target.strip_prefix("cmd/")?;
        (
            SEND_OPEN,
            SEND_CLOSE,
            urlencoding::decode(command).ok()?.into_owned(),
        )
    } else if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
        (A_OPEN, A_CLOSE, dest.to_string())
    } else {
        return None;
    };
    // Decoding can introduce line breaks, terminal controls, or our private delimiters.
    if href.chars().any(|c| c.is_control() || is_link_marker(c)) {
        return None;
    }
    Some((open, close, href))
}

fn push_escaped(out: &mut String, c: char) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        _ => out.push(c),
    }
}

/// Escape rendered text for MXP, turn link markers into tags, and open each line in secure mode.
fn finish_mxp(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len() + rendered.len() / 8);
    for (i, line) in rendered.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.is_empty() {
            continue;
        }
        out.push_str(MXP_SECURE_LINE);
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            let tag = match c {
                SEND_OPEN => "SEND",
                A_OPEN => "A",
                SEND_CLOSE => {
                    out.push_str("</SEND>");
                    continue;
                }
                A_CLOSE => {
                    out.push_str("</A>");
                    continue;
                }
                HREF_END => continue,
                c if c != '"' => {
                    push_escaped(&mut out, c);
                    continue;
                }
                c => {
                    out.push(c);
                    continue;
                }
            };
            out.push('<');
            out.push_str(tag);
            out.push_str(" href=\"");
            for h in chars.by_ref() {
                if h == HREF_END {
                    break;
                }
                push_escaped(&mut out, h);
            }
            out.push_str("\">");
        }
    }
    out
}

/// Tracks list nesting for proper bullet/number rendering
struct ListState {
    kind: ListKind,
    item_number: usize,
}

/// Table being constructed
struct TableState {
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    column_alignments: Vec<Alignment>,
}

impl TableState {
    fn new() -> Self {
        Self {
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: String::new(),
            column_alignments: Vec::new(),
        }
    }
}

/// Definition list being constructed (rendered as a table)
struct DefListState {
    rows: Vec<(String, String)>,
    current_term: String,
    current_details: String,
    in_term: bool,
}

/// Stateful renderer that processes jotdown events into terminal output
struct TerminalRenderer {
    output: String,
    /// Whether the client supports UTF-8 (for fancy characters)
    utf8: bool,
    /// Whether output should be optimized for screen readers / TTS.
    screen_reader_mode: bool,
    /// Style stack for nested inline styles
    style_stack: Vec<StyleModifier>,
    /// List nesting stack
    list_stack: Vec<ListState>,
    /// Current indentation level (for blockquotes, lists, etc.)
    indent_level: usize,
    /// Track if we're at the start of a line (for indentation)
    at_line_start: bool,
    /// Track if we're in a code block
    in_code_block: bool,
    /// Buffered code block content for syntax highlighting
    code_block_content: Option<String>,
    /// Pending newlines to emit (coalesced for better output)
    pending_newlines: usize,
    /// Table state for collecting table data
    table_state: Option<TableState>,
    /// Definition list state for collecting deflist data
    deflist_state: Option<DefListState>,
    markup: Markup,
    /// For each open link, the marker that closes it, if it was written as an MXP element.
    link_stack: Vec<Option<char>>,
}

#[derive(Clone, Copy, Debug)]
enum StyleModifier {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Dimmed,
    FgColor(Color),
    BgColor(Color),
}

impl TerminalRenderer {
    fn new(options: RenderOptions) -> Self {
        Self {
            output: String::new(),
            utf8: options.utf8,
            screen_reader_mode: options.screen_reader_mode,
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            indent_level: 0,
            at_line_start: true,
            in_code_block: false,
            code_block_content: None,
            pending_newlines: 0,
            table_state: None,
            deflist_state: None,
            markup: options.markup,
            link_stack: Vec::new(),
        }
    }

    fn render<'s>(&mut self, events: impl Iterator<Item = Event<'s>>) {
        for event in events {
            self.process_event(event);
        }
        // Flush any pending newlines but trim trailing whitespace
        self.output = self.output.trim_end().to_string();
        if !self.output.is_empty() {
            self.output.push('\n');
        }
    }

    fn process_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(container, _attrs) => self.start_container(container),
            Event::End(container) => self.end_container(container),
            Event::Str(s) => self.render_text(&s),
            Event::FootnoteReference(label) => self.render_footnote_ref(&label),
            Event::Symbol(sym) => self.render_symbol(&sym),
            Event::LeftSingleQuote => self.emit_text(if self.utf8 { "\u{2018}" } else { "'" }),
            Event::RightSingleQuote => self.emit_text(if self.utf8 { "\u{2019}" } else { "'" }),
            Event::LeftDoubleQuote => self.emit_text(if self.utf8 { "\u{201C}" } else { "\"" }),
            Event::RightDoubleQuote => self.emit_text(if self.utf8 { "\u{201D}" } else { "\"" }),
            Event::Ellipsis => self.emit_text(if self.utf8 { "\u{2026}" } else { "..." }),
            Event::EnDash => self.emit_text(if self.utf8 { "\u{2013}" } else { "--" }),
            Event::EmDash => self.emit_text(if self.utf8 { "\u{2014}" } else { "---" }),
            Event::NonBreakingSpace => self.emit_text(if self.utf8 { "\u{00A0}" } else { " " }),
            Event::Softbreak => {
                // In code blocks, soft breaks become newlines; otherwise they're spaces
                if self.in_code_block && self.table_state.is_none() {
                    // If buffering MOO code, add newline to buffer
                    if let Some(buffer) = &mut self.code_block_content {
                        buffer.push('\n');
                    } else {
                        self.emit_newline();
                    }
                } else {
                    self.emit_text(" ");
                }
            }
            Event::Hardbreak => {
                if self.table_state.is_some() {
                    self.emit_text(" ");
                } else {
                    self.emit_newline();
                }
            }
            Event::Blankline => {
                if self.table_state.is_none() {
                    self.pending_newlines = self.pending_newlines.max(2);
                }
            }
            Event::ThematicBreak(_) => {
                if self.screen_reader_mode {
                    self.pending_newlines = self.pending_newlines.max(2);
                } else {
                    self.flush_pending_newlines();
                    self.emit_indent();
                    let rule_char = if self.utf8 { "\u{2500}" } else { "-" };
                    let rule = rule_char.repeat(40);
                    self.output.push_str(&rule.dimmed().to_string());
                    self.pending_newlines = 2;
                }
            }
            Event::Escape | Event::Attributes(_) => {}
        }
    }

    fn start_container(&mut self, container: Container<'_>) {
        match container {
            Container::Paragraph => {
                if self.table_state.is_none() {
                    self.flush_pending_newlines();
                }
            }
            Container::Heading { level, .. } => {
                self.flush_pending_newlines();
                self.emit_indent();
                self.style_stack.push(StyleModifier::Bold);
                match level {
                    1 => self.style_stack.push(StyleModifier::FgColor(Color::Cyan)),
                    2 => self.style_stack.push(StyleModifier::FgColor(Color::Blue)),
                    _ => {}
                }
            }
            Container::Blockquote => {
                self.flush_pending_newlines();
                self.indent_level += 1;
                self.style_stack.push(StyleModifier::Dimmed);
            }
            Container::CodeBlock { language } => {
                self.flush_pending_newlines();
                self.in_code_block = true;
                let lang_str = if language.is_empty() {
                    None
                } else {
                    Some(language.to_string())
                };
                // For MOO code, buffer content for syntax highlighting
                let is_moo = lang_str
                    .as_ref()
                    .is_some_and(|l| l.eq_ignore_ascii_case("moo"));
                if is_moo {
                    self.code_block_content = Some(String::new());
                } else {
                    // Show language label for non-MOO code
                    if let Some(lang) = &lang_str {
                        self.emit_indent();
                        self.output
                            .push_str(&format!("[{}]", lang).dimmed().to_string());
                        self.emit_newline();
                    }
                    self.style_stack.push(StyleModifier::FgColor(Color::White));
                }
            }
            Container::List { kind, tight: _ } => {
                self.flush_pending_newlines();
                self.list_stack.push(ListState {
                    kind,
                    item_number: 0,
                });
            }
            Container::ListItem => {
                self.flush_pending_newlines();
                self.emit_indent();
                if let Some(list_state) = self.list_stack.last_mut() {
                    list_state.item_number += 1;
                    let marker = match &list_state.kind {
                        ListKind::Unordered(_) => {
                            if self.utf8 { "\u{2022} " } else { "* " }.to_string()
                        }
                        ListKind::Ordered {
                            numbering, style, ..
                        } => {
                            let num = list_state.item_number;
                            let num_str = format_ordered_number(num, *numbering);
                            match style {
                                OrderedListStyle::Period => format!("{}. ", num_str),
                                OrderedListStyle::Paren => format!("{}) ", num_str),
                                OrderedListStyle::ParenParen => format!("({}) ", num_str),
                            }
                        }
                        ListKind::Task(_) => {
                            if self.utf8 { "\u{2610} " } else { "[ ] " }.to_string()
                        }
                    };
                    self.output.push_str(&marker.dimmed().to_string());
                }
                self.indent_level += 1;
                self.at_line_start = false;
            }
            Container::TaskListItem { checked } => {
                self.flush_pending_newlines();
                self.emit_indent();
                let marker = if self.utf8 {
                    if checked { "\u{2611} " } else { "\u{2610} " }
                } else if checked {
                    "[x] "
                } else {
                    "[ ] "
                };
                self.output.push_str(&marker.dimmed().to_string());
                self.indent_level += 1;
                self.at_line_start = false;
            }
            Container::DescriptionList => {
                self.flush_pending_newlines();
                self.deflist_state = Some(DefListState {
                    rows: Vec::new(),
                    current_term: String::new(),
                    current_details: String::new(),
                    in_term: false,
                });
            }
            Container::DescriptionTerm => {
                if let Some(state) = &mut self.deflist_state {
                    state.in_term = true;
                    state.current_term = String::new();
                }
            }
            Container::DescriptionDetails => {
                if let Some(state) = &mut self.deflist_state {
                    state.in_term = false;
                    state.current_details = String::new();
                }
            }
            Container::Table => {
                self.flush_pending_newlines();
                self.table_state = Some(TableState::new());
            }
            Container::TableRow { head: _ } => {
                if let Some(state) = &mut self.table_state {
                    state.current_row = Vec::new();
                }
            }
            Container::TableCell { alignment, .. } => {
                if let Some(state) = &mut self.table_state {
                    state.current_cell = String::new();
                    // Track alignment for first row
                    if state.rows.is_empty() && state.current_row.is_empty() {
                        state.column_alignments.push(alignment);
                    }
                }
            }
            Container::Caption => {
                self.flush_pending_newlines();
                self.emit_indent();
                self.style_stack.push(StyleModifier::Italic);
            }
            Container::Footnote { .. } => {
                self.flush_pending_newlines();
                self.indent_level += 1;
            }
            Container::Section { .. } | Container::Div { .. } => {}
            Container::Link(dest, _) => {
                let close = self.open_mxp_link(&dest);
                self.link_stack.push(close);
                self.style_stack.push(StyleModifier::Underline);
                self.style_stack.push(StyleModifier::FgColor(Color::Cyan));
            }
            Container::Image(_, _) => {
                self.emit_text("[");
            }
            Container::Verbatim => {
                self.style_stack.push(StyleModifier::FgColor(Color::Yellow));
            }
            Container::Math { display } => {
                if display && self.table_state.is_none() {
                    self.flush_pending_newlines();
                    self.emit_indent();
                }
                self.style_stack
                    .push(StyleModifier::FgColor(Color::Magenta));
            }
            Container::RawInline { .. } | Container::RawBlock { .. } => {
                self.style_stack.push(StyleModifier::Dimmed);
            }
            Container::Subscript => {
                self.emit_text(if self.utf8 { "\u{208D}" } else { "_(" });
            }
            Container::Superscript => {
                self.emit_text(if self.utf8 { "\u{207D}" } else { "^(" });
            }
            Container::Insert => {
                self.style_stack.push(StyleModifier::FgColor(Color::Green));
            }
            Container::Delete => {
                self.style_stack.push(StyleModifier::Strikethrough);
                self.style_stack.push(StyleModifier::Dimmed);
            }
            Container::Strong => {
                self.style_stack.push(StyleModifier::Bold);
            }
            Container::Emphasis => {
                self.style_stack.push(StyleModifier::Italic);
            }
            Container::Mark => {
                self.style_stack.push(StyleModifier::BgColor(Color::Yellow));
                self.style_stack.push(StyleModifier::FgColor(Color::Black));
            }
            Container::Document | Container::Span | Container::LinkDefinition { .. } => {}
        }
    }

    fn end_container(&mut self, container: Container<'_>) {
        match container {
            Container::Paragraph => {
                if self.table_state.is_none() {
                    self.pending_newlines = self.pending_newlines.max(2);
                }
            }
            Container::Heading { level, .. } => {
                // Pop color if we pushed one
                if level <= 2 {
                    self.style_stack.pop();
                }
                self.style_stack.pop(); // Bold
                self.pending_newlines = 2;
            }
            Container::Blockquote => {
                self.indent_level = self.indent_level.saturating_sub(1);
                self.style_stack.pop();
                self.pending_newlines = 2;
            }
            Container::CodeBlock { .. } => {
                // If we buffered MOO code, render it with syntax highlighting
                if let Some(content) = self.code_block_content.take() {
                    let highlighted = highlight_moo(&content);
                    // Emit with indentation
                    for (i, line) in highlighted.lines().enumerate() {
                        if i > 0 {
                            self.emit_newline();
                        }
                        self.emit_indent();
                        self.output.push_str(line);
                        self.at_line_start = false;
                    }
                } else {
                    self.style_stack.pop();
                }
                self.in_code_block = false;
                self.pending_newlines = 2;
            }
            Container::List { .. } => {
                self.list_stack.pop();
                self.pending_newlines = self.pending_newlines.max(1);
            }
            Container::ListItem | Container::TaskListItem { .. } => {
                self.indent_level = self.indent_level.saturating_sub(1);
                self.pending_newlines = 1;
            }
            Container::DescriptionList => {
                if let Some(state) = self.deflist_state.take() {
                    self.render_deflist(state);
                }
                self.pending_newlines = 2;
            }
            Container::DescriptionTerm => {
                // Term text is collected in deflist_state
            }
            Container::DescriptionDetails => {
                // When details end, save the term+details pair
                if let Some(state) = &mut self.deflist_state {
                    let term = std::mem::take(&mut state.current_term);
                    let details = std::mem::take(&mut state.current_details);
                    state
                        .rows
                        .push((term.trim().to_string(), details.trim().to_string()));
                }
            }
            Container::Table => {
                if let Some(state) = self.table_state.take() {
                    self.render_table(state);
                }
                self.pending_newlines = 2;
            }
            Container::TableRow { head: _ } => {
                if let Some(state) = &mut self.table_state {
                    let row = std::mem::take(&mut state.current_row);
                    state.rows.push(row);
                }
            }
            Container::TableCell { .. } => {
                if let Some(state) = &mut self.table_state {
                    let cell = std::mem::take(&mut state.current_cell);
                    state.current_row.push(cell.trim().to_string());
                }
            }
            Container::Caption => {
                self.style_stack.pop();
                self.pending_newlines = 1;
            }
            Container::Footnote { .. } => {
                self.indent_level = self.indent_level.saturating_sub(1);
                self.pending_newlines = 1;
            }
            Container::Section { .. } | Container::Div { .. } => {}
            Container::Link(_, _) => {
                self.style_stack.pop(); // FgColor
                self.style_stack.pop(); // Underline
                if let Some(close) = self.link_stack.pop().flatten() {
                    self.output.push(close);
                }
            }
            Container::Image(_, _) => {
                self.emit_text("]");
            }
            Container::Verbatim => {
                self.style_stack.pop();
            }
            Container::Math { .. } => {
                self.style_stack.pop();
            }
            Container::RawInline { .. } | Container::RawBlock { .. } => {
                self.style_stack.pop();
            }
            Container::Subscript => {
                self.emit_text(if self.utf8 { "\u{208E}" } else { ")" });
            }
            Container::Superscript => {
                self.emit_text(if self.utf8 { "\u{207E}" } else { ")" });
            }
            Container::Insert => {
                self.style_stack.pop();
            }
            Container::Delete => {
                self.style_stack.pop();
                self.style_stack.pop();
            }
            Container::Strong => {
                self.style_stack.pop();
            }
            Container::Emphasis => {
                self.style_stack.pop();
            }
            Container::Mark => {
                self.style_stack.pop();
                self.style_stack.pop();
            }
            Container::Document | Container::Span | Container::LinkDefinition { .. } => {}
        }
    }

    /// Write the opening marker of an MXP link and return its closing marker. Links in tables
    /// and definition lists stay plain, since markers there would be laid out as text.
    fn open_mxp_link(&mut self, dest: &str) -> Option<char> {
        if self.markup != Markup::Mxp || self.table_state.is_some() || self.deflist_state.is_some()
        {
            return None;
        }
        let (open, close, href) = mxp_link(dest)?;
        self.flush_pending_newlines();
        if self.at_line_start {
            self.emit_indent();
            self.at_line_start = false;
        }
        self.output.push(open);
        self.output.push_str(&href);
        self.output.push(HREF_END);
        Some(close)
    }

    fn render_table(&mut self, state: TableState) {
        if state.rows.is_empty() {
            return;
        }
        if self.screen_reader_mode {
            self.render_table_screen_reader(state);
            return;
        }

        let mut builder = TableBuilder::default();

        for row in &state.rows {
            builder.push_record(row.clone());
        }

        let mut table = builder.build();
        if self.utf8 {
            table.with(Style::rounded());
        } else {
            table.with(Style::ascii());
        }

        // Apply column alignments
        for (i, alignment) in state.column_alignments.iter().enumerate() {
            let tabled_align = match alignment {
                Alignment::Left => TabledAlignment::left(),
                Alignment::Right => TabledAlignment::right(),
                Alignment::Center => TabledAlignment::center(),
                Alignment::Unspecified => TabledAlignment::left(),
            };
            table.with(Modify::new(Columns::new(i..=i)).with(tabled_align));
        }

        // Emit the table with proper indentation
        self.emit_indent();
        let table_str = table.to_string();
        for (i, line) in table_str.lines().enumerate() {
            if i > 0 {
                self.emit_newline();
                self.emit_indent();
            }
            self.output.push_str(line);
        }
        self.at_line_start = false;
    }

    fn render_table_screen_reader(&mut self, state: TableState) {
        let has_headers =
            state.rows.len() > 1 && state.rows[0].iter().any(|cell| !cell.trim().is_empty());
        let headers = has_headers.then(|| state.rows[0].clone());
        let data_rows = if has_headers {
            &state.rows[1..]
        } else {
            &state.rows[..]
        };

        for (row_index, row) in data_rows.iter().enumerate() {
            if row_index > 0 {
                self.emit_newline();
            }
            self.emit_indent();
            let parts: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(col_index, value)| {
                    let label = headers
                        .as_ref()
                        .and_then(|h| h.get(col_index))
                        .filter(|h| !h.trim().is_empty())
                        .cloned()
                        .unwrap_or_else(|| format!("Column {}", col_index + 1));
                    format!("{label}: {value}")
                })
                .collect();
            self.output
                .push_str(&format!("Row {}: {}", row_index + 1, parts.join("; ")));
            self.at_line_start = false;
        }
    }

    fn render_deflist(&mut self, state: DefListState) {
        if state.rows.is_empty() {
            return;
        }
        if self.screen_reader_mode {
            for (i, (term, details)) in state.rows.iter().enumerate() {
                if i > 0 {
                    self.emit_newline();
                }
                self.emit_indent();
                self.output.push_str(&format!("{term}: {details}"));
                self.at_line_start = false;
            }
            return;
        }

        let mut builder = TableBuilder::default();

        for (term, details) in &state.rows {
            builder.push_record([term.clone(), details.clone()]);
        }

        let mut table = builder.build();
        if self.utf8 {
            table.with(Style::rounded());
        } else {
            table.with(Style::ascii());
        }

        // Emit the table with proper indentation
        self.emit_indent();
        let table_str = table.to_string();
        for (i, line) in table_str.lines().enumerate() {
            if i > 0 {
                self.emit_newline();
                self.emit_indent();
            }
            self.output.push_str(line);
        }
        self.at_line_start = false;
    }

    fn render_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        // If we're in a definition list, collect text into term or details
        if let Some(state) = &mut self.deflist_state {
            if state.in_term {
                state.current_term.push_str(text);
            } else {
                state.current_details.push_str(text);
            }
            return;
        }

        // If we're in a table cell, collect text there
        if let Some(state) = &mut self.table_state {
            state.current_cell.push_str(text);
            return;
        }

        // If we're buffering MOO code for syntax highlighting
        if let Some(buffer) = &mut self.code_block_content {
            buffer.push_str(text);
            return;
        }

        if self.in_code_block {
            for (i, line) in text.split('\n').enumerate() {
                if i > 0 {
                    self.emit_newline();
                }
                if !line.is_empty() {
                    self.emit_indent();
                    let styled = self.apply_styles(line);
                    self.output.push_str(&styled);
                    self.at_line_start = false;
                }
            }
        } else {
            self.flush_pending_newlines();
            if self.at_line_start {
                self.emit_indent();
            }
            let styled = self.apply_styles(text);
            self.output.push_str(&styled);
            self.at_line_start = false;
        }
    }

    fn render_footnote_ref(&mut self, label: &str) {
        let styled = format!("[^{}]", label).dimmed().to_string();
        self.emit_text(&styled);
    }

    fn render_symbol(&mut self, symbol: &str) {
        let styled = format!(":{}:", symbol).dimmed().to_string();
        self.emit_text(&styled);
    }

    fn emit_text(&mut self, s: &str) {
        // If we're in a definition list, collect text into term or details
        if let Some(state) = &mut self.deflist_state {
            if state.in_term {
                state.current_term.push_str(s);
            } else {
                state.current_details.push_str(s);
            }
            return;
        }

        // If we're in a table cell, collect text there
        if let Some(state) = &mut self.table_state {
            state.current_cell.push_str(s);
            return;
        }

        if self.at_line_start {
            self.flush_pending_newlines();
            self.emit_indent();
        }
        let styled = self.apply_styles(s);
        self.output.push_str(&styled);
        self.at_line_start = false;
    }

    fn emit_newline(&mut self) {
        self.output.push('\n');
        self.at_line_start = true;
    }

    fn emit_indent(&mut self) {
        if self.indent_level > 0 && self.at_line_start {
            let indent = "  ".repeat(self.indent_level);
            if self
                .style_stack
                .iter()
                .any(|s| matches!(s, StyleModifier::Dimmed))
                && !self.in_code_block
            {
                if self.screen_reader_mode {
                    self.output.push_str(&indent);
                } else {
                    let quote_marker = if self.utf8 { "\u{2502} " } else { "| " };
                    self.output.push_str(&quote_marker.dimmed().to_string());
                    if self.indent_level > 1 {
                        self.output.push_str(&"  ".repeat(self.indent_level - 1));
                    }
                }
            } else {
                self.output.push_str(&indent);
            }
        }
    }

    fn flush_pending_newlines(&mut self) {
        if self.pending_newlines > 0 {
            if !self.output.is_empty() {
                for _ in 0..self.pending_newlines {
                    self.output.push('\n');
                }
            }
            self.pending_newlines = 0;
            self.at_line_start = true;
        }
    }

    fn apply_styles(&self, text: &str) -> String {
        if self.screen_reader_mode {
            return text.to_string();
        }
        if self.style_stack.is_empty() {
            return text.to_string();
        }

        let mut styled: ColoredString = text.into();

        for modifier in &self.style_stack {
            styled = match modifier {
                StyleModifier::Bold => styled.bold(),
                StyleModifier::Italic => styled.italic(),
                StyleModifier::Underline => styled.underline(),
                StyleModifier::Strikethrough => styled.strikethrough(),
                StyleModifier::Dimmed => styled.dimmed(),
                StyleModifier::FgColor(c) => styled.color(*c),
                StyleModifier::BgColor(c) => styled.on_color(*c),
            };
        }

        styled.to_string()
    }
}

fn format_ordered_number(num: usize, numbering: OrderedListNumbering) -> String {
    match numbering {
        OrderedListNumbering::Decimal => num.to_string(),
        OrderedListNumbering::AlphaLower => {
            if num <= 26 {
                char::from_u32('a' as u32 + (num - 1) as u32)
                    .unwrap()
                    .to_string()
            } else {
                num.to_string()
            }
        }
        OrderedListNumbering::AlphaUpper => {
            if num <= 26 {
                char::from_u32('A' as u32 + (num - 1) as u32)
                    .unwrap()
                    .to_string()
            } else {
                num.to_string()
            }
        }
        OrderedListNumbering::RomanLower => to_roman_lower(num),
        OrderedListNumbering::RomanUpper => to_roman_upper(num),
    }
}

fn to_roman_lower(num: usize) -> String {
    to_roman_upper(num).to_lowercase()
}

fn to_roman_upper(mut num: usize) -> String {
    const NUMERALS: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();
    for &(value, numeral) in NUMERALS {
        while num >= value {
            result.push_str(numeral);
            num -= value;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_paragraph() {
        let output = djot_to_terminal("Hello world", true);
        assert!(output.contains("Hello world"));
    }

    #[test]
    fn authored_paragraphs_keep_blank_lines() {
        let input = "# Welcome\n\nFirst paragraph.\n\nSecond paragraph.";
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        let output = djot_to_terminal(input, false);
        assert_eq!(
            ansi.replace_all(&output, ""),
            "Welcome\n\nFirst paragraph.\n\nSecond paragraph.\n"
        );
    }

    #[test]
    fn hard_breaks_are_compact_within_paragraphs() {
        let input =
            "A room description.\n\nYou can go east.\\\nYou see a key here.\\\nHenri is here.";
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        let output = djot_to_terminal(input, false);
        assert_eq!(
            ansi.replace_all(&output, ""),
            "A room description.\n\nYou can go east.\nYou see a key here.\nHenri is here.\n"
        );
    }

    #[test]
    fn code_blocks_keep_internal_blank_lines() {
        let input = "```\nfirst line\n\nthird line\n```";
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        let output = djot_to_terminal(input, false);
        let visible = ansi.replace_all(&output, "");
        assert!(visible.contains("first line\n\nthird line"), "{visible:?}");
    }

    #[test]
    fn test_strong_emphasis() {
        colored::control::set_override(true);
        let output = djot_to_terminal("This is *strong* text", true);
        assert!(output.contains(&"strong".bold().to_string()));
        assert!(!output.contains("*strong*"));
    }

    #[test]
    fn test_emphasis() {
        colored::control::set_override(true);
        let output = djot_to_terminal("This is _emphasized_ text", true);
        assert!(output.contains(&"emphasized".italic().to_string()));
        assert!(!output.contains("_emphasized_"));
    }

    #[test]
    fn test_heading_no_markers() {
        let output = djot_to_terminal("# Heading 1\n\nSome text", true);
        assert!(output.contains("Heading 1"));
        // Should NOT contain the ## markers in output
        assert!(!output.contains("# Heading"));
    }

    #[test]
    fn test_heading_has_ansi_styling() {
        colored::control::set_override(true);
        let output = djot_to_terminal("## Heading 2", true);
        // Should contain ANSI escape codes for styling
        assert!(
            output.contains("\x1b["),
            "Expected ANSI escape codes in heading output: {:?}",
            output
        );
    }

    #[test]
    fn test_definition_list() {
        // In djot, definition lists have the term on one line, then a blank line,
        // then indented definition paragraph
        let djot = r#": Term One

  Definition of term one

: Term Two

  Definition of term two
"#;
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("Term One"));
        assert!(output.contains("Definition of term one"));
        // Should be rendered as a table with rounded borders
        assert!(
            output.contains("│") || output.contains("┌"),
            "Expected table borders in deflist output"
        );
    }

    #[test]
    fn test_bullet_list() {
        let djot = r#"
- Item one
- Item two
- Item three
"#;
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("Item one"));
        assert!(output.contains("•"));
    }

    #[test]
    fn test_code_block() {
        let djot = r#"
```rust
fn main() {
    println!("Hello");
}
```
"#;
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("fn main()"));
        assert!(output.contains("[rust]"));
    }

    #[test]
    fn test_smart_quotes() {
        let djot = r#""Hello," she said."#;
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("\u{201C}") || output.contains("\u{201D}"));
    }

    #[test]
    fn test_link() {
        let djot = "[example](https://example.com)";
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("example"));
        // URL should NOT be shown in terminal output
        assert!(!output.contains("https://example.com"));
    }

    #[test]
    fn test_roman_numerals() {
        assert_eq!(to_roman_upper(1), "I");
        assert_eq!(to_roman_upper(4), "IV");
        assert_eq!(to_roman_upper(9), "IX");
        assert_eq!(to_roman_upper(42), "XLII");
    }

    #[test]
    fn test_blockquote() {
        let djot = "> This is a quote\n> with multiple lines";
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("This is a quote"));
        assert!(output.contains("│"));
    }

    #[test]
    fn test_table() {
        let djot = r#"
| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |
"#;
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("Name"));
        assert!(output.contains("Alice"));
        assert!(output.contains("30"));
        // Should have table borders from tabled
        assert!(output.contains("─") || output.contains("│") || output.contains("┌"));
    }

    #[test]
    fn test_nested_list() {
        let djot = r#"
- Item one

  - Nested item
- Item two
"#;
        let output = djot_to_terminal(djot, true);
        let visible = regex::Regex::new(r"\x1b\[[0-9;]*m")
            .unwrap()
            .replace_all(&output, "")
            .to_string();
        assert!(visible.contains("• Item one"));
        assert!(visible.contains("• Nested item"), "{visible:?}");
        assert!(visible.find("Item one") < visible.find("Nested item"));
        assert!(visible.find("Nested item") < visible.find("Item two"));
        assert!(
            visible
                .lines()
                .find(|line| line.contains("Nested item"))
                .unwrap()
                .starts_with("  •")
        );
    }

    #[test]
    fn test_ordered_list() {
        let djot = r#"
1. First
2. Second
3. Third
"#;
        let output = djot_to_terminal(djot, true);
        let visible = regex::Regex::new(r"\x1b\[[0-9;]*m")
            .unwrap()
            .replace_all(&output, "")
            .to_string();
        assert!(visible.contains("1. First"));
        assert!(visible.contains("2. Second"));
        assert!(visible.contains("3. Third"));
    }

    #[test]
    fn test_thematic_break() {
        let djot = "Before\n\n---\n\nAfter";
        let output = djot_to_terminal(djot, true);
        assert!(output.contains("─"));
    }

    #[test]
    fn test_insert_delete() {
        colored::control::set_override(true);
        let djot = "{+inserted+} and {-deleted-}";
        let output = djot_to_terminal(djot, true);
        assert!(output.contains(&"inserted".color(Color::Green).to_string()));
        assert!(output.contains(&"deleted".strikethrough().dimmed().to_string()));
        assert!(!output.contains("{+"));
        assert!(!output.contains("{-"));
    }

    #[test]
    fn test_mark_highlight() {
        colored::control::set_override(true);
        let djot = "{=highlighted text=}";
        let output = djot_to_terminal(djot, true);
        assert!(output.contains(&"highlighted text".on_yellow().black().to_string()));
        assert!(!output.contains("{="));
    }

    #[test]
    fn test_moo_code_block() {
        colored::control::set_override(true);
        let djot = r#"```moo
set_task_perms(caller_perms());
return $look:mk(this, @this.contents);
```"#;
        let output = djot_to_terminal(djot, true);
        // Should have syntax highlighting (ANSI codes)
        assert!(
            output.contains("\x1b["),
            "MOO code should have ANSI highlighting"
        );
        // Should contain the code
        assert!(output.contains("set_task_perms"));
        assert!(output.contains("return"));
        assert!(output.contains("$look"));
        // Should NOT have the [moo] label since MOO has syntax highlighting
        assert!(
            !output.contains("[moo]"),
            "MOO code block should not show language label"
        );
    }

    #[test]
    fn test_screen_reader_table_format() {
        let djot = r#"
| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |
"#;
        let output = djot_to_terminal_with_options(
            djot,
            RenderOptions {
                utf8: true,
                screen_reader_mode: true,
                ..RenderOptions::default()
            },
        );
        assert!(output.contains("Row 1: Name: Alice; Age: 30"));
        assert!(output.contains("Row 2: Name: Bob; Age: 25"));
        assert!(!output.contains("│"));
        assert!(!output.contains("┌"));
        assert!(!output.contains("─"));
    }

    #[test]
    fn test_screen_reader_disables_ansi_styles() {
        colored::control::set_override(true);
        let output = djot_to_terminal_with_options(
            "# Heading",
            RenderOptions {
                utf8: true,
                screen_reader_mode: true,
                ..RenderOptions::default()
            },
        );
        assert!(!output.contains("\x1b["));
        assert!(output.contains("Heading"));
    }

    fn mxp(input: &str) -> String {
        djot_to_terminal_with_options(
            input,
            RenderOptions {
                utf8: true,
                screen_reader_mode: true,
                markup: Markup::Mxp,
            },
        )
    }

    #[test]
    fn mxp_escapes_and_opens_secure_lines() {
        assert_eq!(
            mxp("a < b & c > d\n\nnext"),
            "\x1b[1za &lt; b &amp; c &gt; d\n\n\x1b[1znext\n"
        );
    }

    #[test]
    fn mxp_command_link_is_send() {
        assert_eq!(
            mxp("[examine sword](moo://cmd/examine%20sword) now"),
            "\x1b[1z<SEND href=\"examine sword\">examine sword</SEND> now\n"
        );
    }

    #[test]
    fn mxp_http_link_is_anchor_with_escaped_href() {
        assert_eq!(
            mxp("see [docs](https://example.com/?a=1&b=\"2\")"),
            "\x1b[1zsee <A href=\"https://example.com/?a=1&amp;b=&quot;2&quot;\">docs</A>\n"
        );
    }

    #[test]
    fn mxp_other_links_are_text() {
        assert_eq!(mxp("[thing](moo://inspect/#1)"), "\x1b[1zthing\n");
    }

    #[test]
    fn mxp_command_links_reject_decoded_controls_and_markers() {
        for suffix in ["%0Alook", "%1B%5B1z", "%EE%80%82", "%EE%80%80"] {
            let input = format!("[label](moo://cmd/look{suffix})");
            assert_eq!(mxp(&input), "\x1b[1zlabel\n");
        }
    }

    #[test]
    fn mxp_markers_in_input_are_dropped() {
        assert_eq!(mxp("x\u{E000}y\u{E002}z"), "\x1b[1zxyz\n");
    }

    #[test]
    fn ansi_markup_has_no_mxp() {
        let output = djot_to_terminal("[look](moo://cmd/look) <b>", true);
        assert!(!output.contains("SEND"));
        assert!(!output.contains("\x1b[1z"));
        assert!(output.contains("<b>"));
    }
}
