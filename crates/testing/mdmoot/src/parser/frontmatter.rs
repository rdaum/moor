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

//! Frontmatter parsing (YAML between --- delimiters)

use crate::ast::Frontmatter;
use eyre::{eyre, Result};
use serde::Deserialize;

/// Raw frontmatter for deserialization
#[derive(Debug, Deserialize, Default)]
struct RawFrontmatter {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    compare: Vec<String>,
}

/// Parse YAML frontmatter from the start of a document
/// Returns (frontmatter, remaining_content)
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, &str)> {
    let content = content.trim_start();

    if !content.starts_with("---") {
        return Ok((Frontmatter::default(), content));
    }

    // Find the closing ---
    let after_open = &content[3..];
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| eyre!("Unclosed frontmatter: missing closing ---"))?;

    let yaml_content = &after_open[..close_pos];
    let body_start = 3 + close_pos + 4; // "---" + yaml + "\n---"
    let body = content[body_start..].trim_start_matches('\n');

    let raw: RawFrontmatter = serde_yaml::from_str(yaml_content.trim())
        .map_err(|e| eyre!("Invalid frontmatter YAML: {}", e))?;

    Ok((
        Frontmatter {
            tags: raw.tags,
            compare: raw.compare,
        },
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_frontmatter() {
        let (fm, body) = parse_frontmatter("# Hello\n\nSome content").unwrap();
        assert!(fm.tags.is_empty());
        assert!(fm.compare.is_empty());
        assert_eq!(body, "# Hello\n\nSome content");
    }

    #[test]
    fn test_with_frontmatter() {
        let content = r#"---
tags: [strings, core]
compare: [moor, lambdamoo]
---

# Test Document
"#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["strings", "core"]);
        assert_eq!(fm.compare, vec!["moor", "lambdamoo"]);
        assert!(body.starts_with("# Test Document"));
    }

    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\n# Hello";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert!(fm.tags.is_empty());
        assert_eq!(body, "# Hello");
    }
}
