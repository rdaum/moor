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

//! Executable documents use top-level fences, not a general Markdown evaluation language.

use super::{Action, parse_at};
use eyre::{Result, WrapErr, eyre};

/// Read every fence deliberately: `moot`, `moo verb=OBJECT:VERB`, or `LANG example`.
/// Borrow bodies unchanged and retain Markdown line numbers for parse and runtime errors.
pub(super) fn parse_markdown(source: &str) -> Result<Vec<(usize, Action<'_>)>> {
    let mut lines = source.split_inclusive('\n').enumerate();
    let mut offset = 0;
    let mut section = "";
    let mut actions = Vec::new();
    while let Some((index, raw)) = lines.next() {
        let line = raw.trim_end_matches(['\r', '\n']);
        offset += raw.len();
        let unindented = line.trim_start_matches([' ', '\t', '>']);
        if unindented != line && (unindented.starts_with("```") || unindented.starts_with("~~~")) {
            return Err(eyre!(
                "line {} ({section}): code fences must start in column one",
                index + 1
            ));
        }
        if let Some(title) = line.strip_prefix('#') {
            let title = title.trim_start_matches('#');
            if title.starts_with(' ') {
                section = title.trim();
                actions.push((index + 1, Action::Section(section)));
            }
        }
        let Some(marker) = line.chars().next().filter(|c| matches!(c, '`' | '~')) else {
            continue;
        };
        let width = line.chars().take_while(|c| *c == marker).count();
        if width < 3 {
            continue;
        }
        let info: Vec<_> = line[width..]
            .trim()
            .splitn(2, char::is_whitespace)
            .map(str::trim)
            .collect();
        let body_start = offset;
        let mut body_end = None;
        for (_, raw) in lines.by_ref() {
            let closing = raw.trim_end();
            if closing.len() >= width && closing.chars().all(|c| c == marker) {
                body_end = Some(offset);
                offset += raw.len();
                break;
            }
            offset += raw.len();
        }
        let body_end =
            body_end.ok_or_else(|| eyre!("line {} ({section}): unclosed code fence", index + 1))?;
        let body = &source[body_start..body_end];
        match info.as_slice() {
            ["moot"] => {
                let steps =
                    parse_at(body, index + 1).wrap_err_with(|| format!("section: {section}"))?;
                if steps.is_empty() {
                    return Err(eyre!("line {} ({section}): empty moot block", index + 1));
                }
                actions.extend(steps);
            }
            ["moo", target] if target.starts_with("verb=") => {
                let verb = &target[5..];
                let valid = verb
                    .split_once(':')
                    .is_some_and(|(object, name)| !object.is_empty() && !name.is_empty());
                if !valid || body.trim().is_empty() {
                    return Err(eyre!(
                        "line {} ({section}): expected moo verb=OBJECT:VERB and a nonempty body",
                        index + 1
                    ));
                }
                actions.push((index + 1, Action::Program { verb, body }));
            }
            [_, "example"] => (),
            _ => {
                return Err(eyre!(
                    "line {} ({section}): unknown code fence; use moot, moo verb=OBJECT:VERB, or LANG example",
                    index + 1
                ));
            }
        }
    }
    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::tests::{Runner, actors};
    use crate::scenario::{Expectation, execute_actions};
    use std::collections::VecDeque;

    #[test]
    fn document_preserves_actor_body_order_and_line_numbers() {
        let source = "# First\n\n```moot\n@programmer\n% create\n```\nProse.\n\
                      ## Program\n```moo verb=rock:pet\n  return 1;\n.comment\n```\n\
                      ```moot\n=compiled\n; return 7;\n7\n```\n";
        let actions = parse_markdown(source).unwrap();
        assert!(matches!(actions[2], (5, Action::Test { .. })));
        assert_eq!(
            actions[4],
            (
                9,
                Action::Program {
                    verb: "rock:pet",
                    body: "  return 1;\n.comment\n"
                }
            )
        );
        let mut runner = Runner {
            output: VecDeque::from(["compiled".into()]),
            ..Default::default()
        };
        execute_actions(&mut runner, &actors(), actions).unwrap();
        assert_eq!(
            runner.calls[..5],
            [
                (actors().programmer, "create".into()),
                (actors().programmer, "input:  return 1;".into()),
                (actors().programmer, "input:..comment".into()),
                (actors().programmer, "input:.".into()),
                (actors().programmer, "@program rock:pet".into()),
            ]
        );
        assert_eq!(runner.calls[5].0, actors().programmer);
    }

    #[test]
    fn quoted_object_name_reaches_the_program_command() {
        let source = "```moo verb=\"pet rock\":pet\nreturn true;\n```\n";
        let actions = parse_markdown(source).unwrap();
        let mut runner = Runner::default();
        execute_actions(&mut runner, &actors(), actions).unwrap();
        assert_eq!(
            runner.calls.last(),
            Some(&(actors().wizard, "@program \"pet rock\":pet".into()))
        );
    }

    #[test]
    fn examples_cannot_execute_and_nested_fences_do_not_close_early() {
        let source = "~~~~text example\n```moot\n% bad\n```\n~~~~\n\
                      # Real\n````moot\n% command\n=```\n````\n";
        let actions = parse_markdown(source).unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[1].1, Action::Test { expected, .. }
            if expected == &vec![(9, Expectation::Exact("```"))]));
    }

    #[test]
    fn malformed_and_empty_documents_fail() {
        for source in [
            "# Broken\n```mooot\n% bad\n```",
            "# Broken\n```moo\nreturn 1;\n```",
            "# Broken\n```moot typo\n% bad\n```",
            "# Broken\n```moo verb=rock\nreturn 1;\n```",
            "# Broken\n```moo verb=rock:pet\n```",
            "# Broken\n```moot\n% bad",
            "# Broken\n```moot\norphan\n```",
            "# Broken\n```moot\n// no steps\n```",
            "# Broken\n  ```moot\n% bad\n  ```",
            "# Broken\n> ```moot\n> % bad\n> ```",
        ] {
            let error = format!("{:?}", parse_markdown(source).unwrap_err());
            assert!(
                error.contains("Broken") && error.contains("line"),
                "{error}"
            );
        }
        for source in ["Only prose.", "```moo example\nreturn 1;\n```"] {
            let actions = parse_markdown(source).unwrap();
            assert!(execute_actions(&mut Runner::default(), &actors(), actions).is_err());
        }
    }

    #[test]
    fn failures_name_the_document_section_and_original_line() {
        let source = "# Start\n```moot\n% command\n```\n\n## Petting\n\
                      ```moot\n% bad\n```\n";
        let error = execute_actions(
            &mut Runner::default(),
            &actors(),
            parse_markdown(source).unwrap(),
        )
        .unwrap_err();
        let message = format!("{error:?}");
        assert!(message.contains("line 8 (Petting)"), "{message}");
        assert!(message.contains("underlying command error"), "{message}");

        let source = source.replace("% bad", "% command\n=expected output");
        let error = execute_actions(
            &mut Runner::default(),
            &actors(),
            parse_markdown(&source).unwrap(),
        )
        .unwrap_err();
        let message = format!("{error:?}");
        assert!(
            message.contains("line 8 (Petting)") && message.contains("line 9"),
            "{message}"
        );
        assert!(message.contains("expected output"), "{message}");
    }
}
