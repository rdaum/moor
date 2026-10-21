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

//! Syntax highlighting for MOO code using regex-based tokenization and ANSI colors.

use colored::{Color, Colorize};
use std::sync::LazyLock;

/// Application-provided highlighting for MOO fences in termjot documents.
pub(super) struct MooHighlighter;

impl termjot::SyntaxHighlighter for MooHighlighter {
    fn highlight(&self, language: &str, code: &str) -> Option<String> {
        language
            .eq_ignore_ascii_case("moo")
            .then(|| highlight_moo(code))
    }
}

/// Token types for MOO syntax
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenType {
    Keyword,
    String,
    Comment,
    Number,
    Object,
    Error,
    SysProp,
    Symbol,
    TypeConstant,
    Operator,
    Plain,
}

/// A token with its type and text
struct Token<'a> {
    token_type: TokenType,
    text: &'a str,
}

/// Regex patterns for MOO tokens
struct MooPatterns {
    comment_line: regex::Regex,
    comment_block_start: regex::Regex,
    keyword: regex::Regex,
    type_constant: regex::Regex,
    error_code: regex::Regex,
    sysprop: regex::Regex,
    object_uuid: regex::Regex,
    object_num: regex::Regex,
    symbol: regex::Regex,
    float: regex::Regex,
    integer: regex::Regex,
    string: regex::Regex,
    binary_string: regex::Regex,
    boolean: regex::Regex,
}

static PATTERNS: LazyLock<MooPatterns> = LazyLock::new(|| {
    MooPatterns {
    comment_line: regex::Regex::new(r"^//.*").unwrap(),
    comment_block_start: regex::Regex::new(r"^/\*").unwrap(),
    keyword: regex::Regex::new(
        r"(?i)^(if|elseif|else|endif|for|endfor|while|endwhile|in|try|except|endtry|finally|return|break|continue|pass|raise|fork|endfork|begin|end|fn|endfn|let|const|global|any)\b",
    )
    .unwrap(),
    type_constant: regex::Regex::new(
        r"(?i)^(INT|NUM|FLOAT|STR|ERR|OBJ|LIST|MAP|BOOL|FLYWEIGHT|BINARY|LAMBDA|SYM)\b",
    )
    .unwrap(),
    error_code: regex::Regex::new(r"(?i)^E_[A-Z_]+").unwrap(),
    sysprop: regex::Regex::new(r"^\$[a-zA-Z_][a-zA-Z0-9_]*").unwrap(),
    object_uuid: regex::Regex::new(r"^#(anon_)?[0-9A-Fa-f]{6}-[0-9A-Fa-f]{10}").unwrap(),
    object_num: regex::Regex::new(r"^#-?\d+").unwrap(),
    symbol: regex::Regex::new(r"^'[a-zA-Z_][a-zA-Z0-9_]*").unwrap(),
    // Float patterns: decimal with optional exponent, exponent only, or trailing decimal
    // Note: we can't use lookahead, so trailing decimal (like 5.) will be handled in context
    float: regex::Regex::new(r"^[-+]?\d*\.\d+([eE][-+]?\d+)?|^[-+]?\d+[eE][-+]?\d+").unwrap(),
    integer: regex::Regex::new(r"^[-+]?\d+").unwrap(),
    string: regex::Regex::new(r#"^"([^"\\]|\\.)*""#).unwrap(),
    binary_string: regex::Regex::new(r#"^b"[A-Za-z0-9+/=_-]*""#).unwrap(),
    boolean: regex::Regex::new(r"^(true|false)\b").unwrap(),
}
});

/// Highlight MOO code with ANSI terminal colors.
pub fn highlight_moo(code: &str) -> String {
    let mut output = String::new();
    let mut in_block_comment = false;

    for (line_number, line) in code.split('\n').enumerate() {
        if line_number > 0 {
            output.push('\n');
        }

        let mut pos = 0;
        while pos < line.len() {
            let remaining = &line[pos..];

            // Handle block comment continuation
            if in_block_comment {
                if let Some(end_pos) = remaining.find("*/") {
                    output.push_str(&colorize(&remaining[..end_pos + 2], TokenType::Comment));
                    pos += end_pos + 2;
                    in_block_comment = false;
                } else {
                    output.push_str(&colorize(remaining, TokenType::Comment));
                    break;
                }
                continue;
            }

            // Try to match tokens
            if let Some(token) = try_match_token(remaining) {
                output.push_str(&colorize(token.text, token.token_type));
                pos += token.text.len();

                // Check if we started a block comment that doesn't close on this line
                if token.token_type == TokenType::Comment
                    && token.text.starts_with("/*")
                    && !token.text.ends_with("*/")
                {
                    in_block_comment = true;
                }
            } else {
                // No match - output single character
                let ch = remaining.chars().next().unwrap();
                output.push(ch);
                pos += ch.len_utf8();
            }
        }
    }

    output
}

fn try_match_token(text: &str) -> Option<Token<'_>> {
    // Skip whitespace - return as plain
    if text.starts_with(char::is_whitespace) {
        let end = text
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(text.len());
        return Some(Token {
            token_type: TokenType::Plain,
            text: &text[..end],
        });
    }

    // Line comment
    if let Some(m) = PATTERNS.comment_line.find(text) {
        return Some(Token {
            token_type: TokenType::Comment,
            text: m.as_str(),
        });
    }

    // Block comment start
    if PATTERNS.comment_block_start.is_match(text) {
        // Find end of block comment
        if let Some(end_pos) = text[2..].find("*/") {
            return Some(Token {
                token_type: TokenType::Comment,
                text: &text[..end_pos + 4],
            });
        } else {
            return Some(Token {
                token_type: TokenType::Comment,
                text,
            });
        }
    }

    // Binary string (before regular string)
    if let Some(m) = PATTERNS.binary_string.find(text) {
        return Some(Token {
            token_type: TokenType::String,
            text: m.as_str(),
        });
    }

    // String
    if let Some(m) = PATTERNS.string.find(text) {
        return Some(Token {
            token_type: TokenType::String,
            text: m.as_str(),
        });
    }

    // Keywords (before identifiers)
    if let Some(m) = PATTERNS.keyword.find(text) {
        return Some(Token {
            token_type: TokenType::Keyword,
            text: m.as_str(),
        });
    }

    // Type constants
    if let Some(m) = PATTERNS.type_constant.find(text) {
        return Some(Token {
            token_type: TokenType::TypeConstant,
            text: m.as_str(),
        });
    }

    // Boolean literals
    if let Some(m) = PATTERNS.boolean.find(text) {
        return Some(Token {
            token_type: TokenType::TypeConstant,
            text: m.as_str(),
        });
    }

    // Error codes
    if let Some(m) = PATTERNS.error_code.find(text) {
        return Some(Token {
            token_type: TokenType::Error,
            text: m.as_str(),
        });
    }

    // System properties/objects
    if let Some(m) = PATTERNS.sysprop.find(text) {
        return Some(Token {
            token_type: TokenType::SysProp,
            text: m.as_str(),
        });
    }

    // Object UUIDs
    if let Some(m) = PATTERNS.object_uuid.find(text) {
        return Some(Token {
            token_type: TokenType::Object,
            text: m.as_str(),
        });
    }

    // Object numbers
    if let Some(m) = PATTERNS.object_num.find(text) {
        return Some(Token {
            token_type: TokenType::Object,
            text: m.as_str(),
        });
    }

    // Symbols
    if let Some(m) = PATTERNS.symbol.find(text) {
        return Some(Token {
            token_type: TokenType::Symbol,
            text: m.as_str(),
        });
    }

    // Floats (before integers)
    if let Some(m) = PATTERNS.float.find(text) {
        return Some(Token {
            token_type: TokenType::Number,
            text: m.as_str(),
        });
    }

    // Integers
    if let Some(m) = PATTERNS.integer.find(text) {
        return Some(Token {
            token_type: TokenType::Number,
            text: m.as_str(),
        });
    }

    // Operators - just return single char for simplicity
    let first = text.chars().next()?;
    if "+-*/%^=!<>&|~@:;,.(){}[]`'?".contains(first) {
        return Some(Token {
            token_type: TokenType::Operator,
            text: &text[..first.len_utf8()],
        });
    }

    // Identifier or unknown - consume word
    let end = text
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(text.len());
    if end > 0 {
        return Some(Token {
            token_type: TokenType::Plain,
            text: &text[..end],
        });
    }

    None
}

fn colorize(text: &str, token_type: TokenType) -> String {
    match token_type {
        TokenType::Keyword => text.bold().color(Color::Magenta).to_string(),
        TokenType::String => text.color(Color::Green).to_string(),
        TokenType::Comment => text.dimmed().to_string(),
        TokenType::Number => text.color(Color::Yellow).to_string(),
        TokenType::Object => text.color(Color::Cyan).to_string(),
        TokenType::Error => text.color(Color::Red).to_string(),
        TokenType::SysProp => text.color(Color::BrightYellow).to_string(),
        TokenType::Symbol => text.color(Color::Blue).to_string(),
        TokenType::TypeConstant => text.color(Color::Cyan).to_string(),
        TokenType::Operator => text.to_string(),
        TokenType::Plain => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termjot_highlighter_preserves_unicode_and_blank_lines() {
        use termjot::SyntaxHighlighter;

        colored::control::set_override(true);
        let source = "\n\nreturn \"café\"; /* é\né */ return 42;\n\n";
        let output = MooHighlighter.highlight("MoO", source).unwrap();
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        assert_eq!(ansi.replace_all(&output, ""), source);
        assert!(MooHighlighter.highlight("rust", source).is_none());
        assert!(MooHighlighter.highlight("", source).is_none());
    }

    #[test]
    fn highlights_token_classes_without_changing_source() {
        colored::control::set_override(true);
        let cases = [
            (
                "return x;",
                "return".bold().color(Color::Magenta).to_string(),
            ),
            (
                r#"x = "hello";"#,
                "\"hello\"".color(Color::Green).to_string(),
            ),
            ("#0.name", "#0".color(Color::Cyan).to_string()),
            ("raise(E_PERM);", "E_PERM".color(Color::Red).to_string()),
            (
                "$string_utils:capitalize(s);",
                "$string_utils".color(Color::BrightYellow).to_string(),
            ),
            ("x = 'foo;", "'foo".color(Color::Blue).to_string()),
            ("typeof(x) == INT", "INT".color(Color::Cyan).to_string()),
            ("// comment", "// comment".dimmed().to_string()),
        ];
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        for (source, styled_token) in cases {
            let output = highlight_moo(source);
            assert_eq!(ansi.replace_all(&output, ""), source);
            assert!(
                output.contains(&styled_token),
                "missing style for {source:?}: {output:?}"
            );
        }
    }

    #[test]
    fn block_comment_stays_dimmed_across_lines() {
        colored::control::set_override(true);
        let source = "x = 5; /* this is\na multi-line\ncomment */ y = 6;";
        let output = highlight_moo(source);
        let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
        assert_eq!(ansi.replace_all(&output, ""), source);
        for line in ["/* this is", "a multi-line", "comment */"] {
            assert!(
                output.contains(&line.dimmed().to_string()),
                "comment line {line:?} lost its style: {output:?}"
            );
        }
    }
}
