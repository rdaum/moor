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

//! Syntax highlighting for MOO code using regex-based tokenization and ANSI colors.

use colored::{Color, Colorize};
use once_cell::sync::Lazy;

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

static PATTERNS: Lazy<MooPatterns> = Lazy::new(|| {
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
    // Force colors on since we're generating output for telnet, not stdout
    colored::control::set_override(true);

    let mut output = String::new();
    let mut in_block_comment = false;

    for line in code.lines() {
        if !output.is_empty() {
            output.push('\n');
        }

        let mut pos = 0;
        let chars: Vec<char> = line.chars().collect();

        while pos < chars.len() {
            let remaining: String = chars[pos..].iter().collect();

            // Handle block comment continuation
            if in_block_comment {
                if let Some(end_pos) = remaining.find("*/") {
                    let comment_text: String = chars[pos..pos + end_pos + 2].iter().collect();
                    output.push_str(&colorize(&comment_text, TokenType::Comment));
                    pos += end_pos + 2;
                    in_block_comment = false;
                } else {
                    output.push_str(&colorize(&remaining, TokenType::Comment));
                    break;
                }
                continue;
            }

            // Try to match tokens
            if let Some(token) = try_match_token(&remaining) {
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
                output.push(chars[pos]);
                pos += 1;
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
    fn test_highlight_basic_moo() {
        let code = r#"set_task_perms(caller_perms());
return $look:mk(this, @this.contents);"#;

        let highlighted = highlight_moo(code);

        // Should contain ANSI escape sequences
        assert!(highlighted.contains("\x1b["));
        // Should still contain the original code text
        assert!(highlighted.contains("set_task_perms"));
        assert!(highlighted.contains("caller_perms"));
        assert!(highlighted.contains("return"));
    }

    #[test]
    fn test_highlight_keywords() {
        let code = "if (x > 0)\n  return x;\nendif";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("if"));
        assert!(highlighted.contains("return"));
        assert!(highlighted.contains("endif"));
    }

    #[test]
    fn test_highlight_strings() {
        let code = r#"x = "hello world";"#;
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("hello world"));
    }

    #[test]
    fn test_highlight_objects() {
        let code = "#0.name = \"Root\";\nplayer = #-1;";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("#0"));
        assert!(highlighted.contains("#-1"));
    }

    #[test]
    fn test_highlight_comments() {
        let code = "// This is a comment\nx = 5; /* inline */";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("This is a comment"));
    }

    #[test]
    fn test_highlight_sysprop() {
        let code = "$string_utils:capitalize(s);";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("$string_utils"));
    }

    #[test]
    fn test_highlight_errors() {
        let code = "raise(E_PERM);";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("E_PERM"));
    }

    #[test]
    fn test_highlight_symbols() {
        let code = "x = 'foo;";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("'foo"));
    }

    #[test]
    fn test_highlight_type_constants() {
        let code = "typeof(x) == INT";
        let highlighted = highlight_moo(code);

        assert!(highlighted.contains("\x1b["));
        assert!(highlighted.contains("INT"));
    }

    #[test]
    fn test_highlight_block_comment() {
        let code = "x = 5; /* this is\na multi-line\ncomment */ y = 6;";
        let highlighted = highlight_moo(code);

        // The multi-line comment should all be styled the same
        assert!(highlighted.contains("multi-line"));
    }
}
