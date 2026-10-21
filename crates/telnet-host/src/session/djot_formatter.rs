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

//! Connect termjot's terminal renderer to mooR's MOO syntax highlighter.

use super::moo_highlighter::MooHighlighter;
pub(crate) use termjot::Markup;

/// Connection-specific layout settings, safe to retain across async writes.
///
/// The borrowed highlighter is attached only during synchronous rendering.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RenderOptions {
    pub(crate) utf8: bool,
    pub(crate) screen_reader_mode: bool,
    pub(crate) markup: Markup,
}

/// Render Djot using the connection's layout settings and mooR's highlighter.
pub(crate) fn djot_to_terminal_with_options(input: &str, options: RenderOptions) -> String {
    termjot::djot_to_terminal_with_options(
        input,
        termjot::RenderOptions {
            utf8: options.utf8,
            screen_reader_mode: options.screen_reader_mode,
            markup: options.markup,
            syntax_highlighter: Some(&MooHighlighter),
        },
    )
}

#[cfg(test)]
fn djot_to_terminal(input: &str, utf8: bool) -> String {
    djot_to_terminal_with_options(
        input,
        RenderOptions {
            utf8,
            ..RenderOptions::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use colored::{Color, Colorize};

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
