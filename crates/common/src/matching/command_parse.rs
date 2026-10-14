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

use std::{marker::PhantomData, string::ToString};

use crate::{
    matching::{
        CommandParser, ObjectNameMatcher, ParseCommandError, ParsedCommand, all_prepositions,
        find_preposition_for_command, get_preposition_forms,
    },
    model::PrepSpec,
    util,
};
use moor_var::{Var, v_str};

/// The default command parser implementation based on the one in LambdaMOO 1.8.
pub struct DefaultParseCommand<M>
where
    M: ObjectNameMatcher,
{
    phantom_data: PhantomData<M>,
}

impl<M> Default for DefaultParseCommand<M>
where
    M: ObjectNameMatcher,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<M> DefaultParseCommand<M>
where
    M: ObjectNameMatcher,
{
    pub fn new() -> Self {
        DefaultParseCommand {
            phantom_data: PhantomData,
        }
    }
}

impl<M> CommandParser<M> for DefaultParseCommand<M>
where
    M: ObjectNameMatcher,
{
    fn parse_command(
        &self,
        input: &str,
        command_environment: &M,
    ) -> Result<ParsedCommand, ParseCommandError> {
        // Replace initial command characters with say/emote/eval
        let mut command = input.trim_start().to_string();
        let first_char = command.chars().next().unwrap_or(' ');
        match first_char {
            '"' => command.replace_range(..1, "say "),
            ':' => command.replace_range(..1, "emote "),
            ';' => command.replace_range(..1, "eval "),
            _ => {}
        };

        let scanned = util::command_words(&command);
        let Some(first) = scanned.first() else {
            return Err(ParseCommandError::EmptyCommand);
        };
        let verb = command[first.bytes.clone()].into();
        // Remove the separating character, retaining the rest of the argument exactly.
        let tail = &command[first.bytes.end..];
        let argstr = tail
            .chars()
            .next()
            .map_or("", |ch| &tail[ch.len_utf8()..])
            .to_string();
        let tokens = util::command_words(&argstr);
        let words: Vec<String> = tokens.iter().map(|word| word.text.clone()).collect();
        let (prep_match, prep) = seek_preposition(&tokens);

        // Get direct object string
        let dobjstr = match prep_match {
            Some((j, _)) => {
                if j == 0 {
                    None
                } else {
                    Some(words[0..j].join(" "))
                }
            }
            None => {
                if words.is_empty() {
                    None
                } else {
                    Some(words.join(" "))
                }
            }
        };
        let (dobj, ambiguous_dobj) = match &dobjstr {
            Some(dobjstr) => {
                let match_result = command_environment
                    .match_object(dobjstr)
                    .map_err(ParseCommandError::ErrorDuringMatch)?;
                let candidates = if !match_result.candidates.is_empty() {
                    Some(match_result.candidates)
                } else {
                    None
                };
                (match_result.result, candidates)
            }
            None => (None, None),
        };

        // Get indirect object string
        let iobjstr = prep_match.map(|(_, end)| words[end..].join(" "));

        // Get indirect object object
        let (iobj, ambiguous_iobj) = match (prep, &iobjstr) {
            (PrepSpec::None, _) => (None, None),
            (_, Some(iobjstr)) => {
                let match_result = command_environment
                    .match_object(iobjstr)
                    .map_err(ParseCommandError::ErrorDuringMatch)?;
                let candidates = if !match_result.candidates.is_empty() {
                    Some(match_result.candidates)
                } else {
                    None
                };
                (match_result.result, candidates)
            }
            _ => (None, None),
        };

        // Build and return ParsedCommand
        let args: Vec<Var> = words.iter().map(|w| v_str(w)).collect();

        Ok(ParsedCommand {
            verb,
            argstr,
            args,
            dobjstr,
            dobj,
            ambiguous_dobj,
            prepstr: prep_match.map(|(start, end)| words[start..end].join(" ")),
            prep,
            iobjstr,
            iobj,
            ambiguous_iobj,
        })
    }
}
// Match the earliest preposition, preferring the longest spelling at that position.
// Token bounds keep every word of a multiword preposition out of the object strings.
pub fn seek_preposition(words: &[util::CommandWord]) -> (Option<(usize, usize)>, PrepSpec) {
    for (start, word) in words.iter().enumerate() {
        if word.quoted {
            continue;
        }
        let mut best = find_preposition_for_command(&word.text).map(|prep| (1, prep));
        for prep in all_prepositions() {
            for form in get_preposition_forms(prep) {
                let mut length = 0;
                let matches = form.split(' ').all(|part| {
                    let matches = words
                        .get(start + length)
                        .is_some_and(|word| !word.quoted && word.text.eq_ignore_ascii_case(part));
                    length += 1;
                    matches
                });
                if matches && best.is_none_or(|(best_length, _)| length > best_length) {
                    best = Some((length, prep));
                }
            }
        }
        if let Some((length, prep)) = best {
            return (Some((start, start + length)), PrepSpec::Other(prep));
        }
    }
    (None, PrepSpec::None)
}

#[cfg(test)]
mod tests {
    use crate::{model::WorldStateError, util::parse_into_words};
    use moor_var::{FAILED_MATCH, Obj, v_str};

    use super::*;
    use crate::matching::{
        ObjectNameMatcher, Preposition,
        match_env::DefaultObjectNameMatcher,
        mock_matching_env::{
            MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2, setup_mock_environment,
        },
    };

    #[test]
    fn quoted_names_and_scanner_positions() {
        let input = "\t\"é  rock\"\t\"\"  a\\ b";
        let words = util::command_words(input);
        assert_eq!(
            words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(),
            ["é  rock", "", "a b"]
        );
        assert_eq!(words[0].chars, 1..10);
        assert_eq!(&input[words[0].bytes.clone()], "\"é  rock\"");
        assert!(words.iter().all(|word| word.quoted));
        let parser = DefaultParseCommand::new();
        let parsed = parser
            .parse_command("look\tobj", &SimpleParseMatcher {})
            .unwrap();
        assert_eq!(parsed.verb.as_string(), "look");
        assert_eq!(parsed.dobj, Some(Obj::mk_id(1)));
        let parsed = parser
            .parse_command("rename \"to\" to player", &SimpleParseMatcher {})
            .unwrap();
        assert_eq!(parsed.dobjstr.as_deref(), Some("to"));
        assert_eq!(parsed.iobjstr.as_deref(), Some("player"));
        let parsed = parser
            .parse_command("#\"Dr. Moss\".name", &SimpleParseMatcher {})
            .unwrap();
        assert_eq!(parsed.verb.as_string(), "#\"Dr. Moss\".name");
    }

    #[test]
    fn test_parse_into_words_simple() {
        let input = "hello world";
        let expected_output = vec!["hello", "world"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    #[test]
    fn test_parse_into_words_double_quotes() {
        let input = "hello \"big world\"";
        let expected_output = vec!["hello", "big world"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    #[test]
    fn test_parse_into_words_backslash() {
        let input = r"hello\ world frankly";
        let expected_output = vec!["hello world", "frankly"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    #[test]
    fn test_parse_into_words_mixed() {
        let input = r#"hello "big world"\" foo bar\"baz"#;
        let expected_output = vec!["hello", "big world\"", "foo", "bar\"baz"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    use crate::matching::MatchResult;

    struct SimpleParseMatcher {}
    impl ObjectNameMatcher for SimpleParseMatcher {
        fn match_object(&self, name: &str) -> Result<MatchResult, WorldStateError> {
            let result = match name {
                "obj" => Some(Obj::mk_id(1)),
                "player" => Some(Obj::mk_id(2)),
                _ => None,
            };
            Ok(MatchResult {
                result,
                candidates: Vec::new(),
            })
        }
    }

    #[test]
    fn test_parse_single_arg_command() {
        // Test normal MOO command
        let command = "look obj";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "look");
        assert_eq!(parsed.dobjstr, Some("obj".to_string()));
        assert_eq!(parsed.dobj, Some(Obj::mk_id(1)));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.iobj, None);
        assert_eq!(parsed.args, vec![v_str("obj")]);
        assert_eq!(parsed.argstr, "obj");
    }

    #[test]
    fn test_parse_multi_arg_command() {
        // Test normal MOO command with multiple args
        let command = "test arg1 arg2 arg3";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();

        assert_eq!(parsed.verb.as_string(), "test");
        assert_eq!(parsed.dobjstr, Some("arg1 arg2 arg3".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
        assert_eq!(
            parsed.args,
            vec![v_str("arg1"), v_str("arg2"), v_str("arg3")]
        );
        assert_eq!(parsed.argstr, "arg1 arg2 arg3");
    }

    #[test]
    fn test_parse_dobj_prep_iobj_command() {
        // Test command with prep
        let command = "give obj to player";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "give");
        assert_eq!(parsed.dobjstr, Some("obj".to_string()));
        assert_eq!(parsed.dobj, Some(Obj::mk_id(1)));
        assert_eq!(parsed.prepstr, Some("to".to_string()));
        assert_eq!(parsed.prep, PrepSpec::Other(Preposition::AtTo));
        assert_eq!(parsed.iobjstr, Some("player".to_string()));
        assert_eq!(parsed.iobj, Some(Obj::mk_id(2)));
        assert_eq!(
            parsed.args,
            vec![v_str("obj"), v_str("to"), v_str("player")]
        );
        assert_eq!(parsed.argstr, "obj to player");
    }

    #[test]
    fn test_parse_every_preposition_spelling() {
        let parser = DefaultParseCommand::new();
        for prep in all_prepositions() {
            for form in get_preposition_forms(prep) {
                for direct in ["", "obj "] {
                    let parsed = parser
                        .parse_command(
                            &format!("test {direct}{form} player"),
                            &SimpleParseMatcher {},
                        )
                        .unwrap();
                    assert_eq!(parsed.prep, PrepSpec::Other(prep), "{form}");
                    assert_eq!(parsed.prepstr.as_deref(), Some(*form));
                    assert_eq!(parsed.dobj, (!direct.is_empty()).then_some(Obj::mk_id(1)));
                    assert_eq!(parsed.iobjstr.as_deref(), Some("player"));
                    assert_eq!(parsed.iobj, Some(Obj::mk_id(2)));
                }
            }
        }
    }

    #[test]
    fn test_preposition_boundaries_and_incomplete_phrases() {
        let parser = DefaultParseCommand::new();
        for (command, prep, indirect) in [
            ("test obj on top", Some("on"), Some("top")),
            ("test obj out of", Some("out of"), Some("")),
            ("test obj out offstage", None, None),
            ("test obj in front office", Some("in"), Some("front office")),
            (
                "test obj to player on top of obj",
                Some("to"),
                Some("player on top of obj"),
            ),
            (
                "test obj on   top  of player",
                Some("on top of"),
                Some("player"),
            ),
            ("test obj \"on top of\" player", None, None),
        ] {
            let parsed = parser
                .parse_command(command, &SimpleParseMatcher {})
                .unwrap();
            assert_eq!(parsed.prepstr.as_deref(), prep, "{command}");
            assert_eq!(parsed.iobjstr.as_deref(), indirect, "{command}");
        }
    }

    #[test]
    fn test_parse_say_abbrev_command() {
        // Test say abbrev command
        let command = "\"hello, world!";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "say");
        assert_eq!(parsed.dobjstr, Some("hello, world!".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("hello,"), v_str("world!")]);
        assert_eq!(parsed.argstr, "hello, world!");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_parse_emote_command() {
        // Test emote command
        let command = ":waves happily.";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "emote");
        assert_eq!(parsed.dobjstr, Some("waves happily.".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("waves"), v_str("happily.")]);
        assert_eq!(parsed.argstr, "waves happily.");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_parse_emote_explicit_command() {
        // Test emote command
        let command = "emote waves happily.";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "emote");
        assert_eq!(parsed.dobjstr, Some("waves happily.".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("waves"), v_str("happily.")]);
        assert_eq!(parsed.argstr, "waves happily.");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_parse_eval_command() {
        // Test eval command
        let command = ";1 + 1";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "eval");
        assert_eq!(parsed.dobjstr, Some("1 + 1".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("1"), v_str("+"), v_str("1")]);
        assert_eq!(parsed.argstr, "1 + 1");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_parse_quoted_arg_command() {
        // Test command with single escaped argument
        let command = "blork \"hello, world!\"";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "blork");
        assert_eq!(parsed.dobjstr, Some("hello, world!".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("hello, world!")]);
        assert_eq!(parsed.argstr, "\"hello, world!\"");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_parse_say_abbrev_quoted_command() {
        // Test say abbrev command
        let command = "\"\"hello, world!\"";
        let pc = DefaultParseCommand::new();
        let parsed = pc.parse_command(command, &SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_string(), "say");
        assert_eq!(parsed.dobjstr, Some("hello, world!".to_string()));
        assert_eq!(parsed.prepstr, None);
        assert_eq!(parsed.iobjstr, None);
        assert_eq!(parsed.args, vec![v_str("hello, world!")]);
        assert_eq!(parsed.argstr, "\"hello, world!\"");
        assert_eq!(parsed.dobj, None);
        assert_eq!(parsed.iobj, None);
    }

    #[test]
    fn test_command_parser_get_thing1() {
        let env = setup_mock_environment();
        let match_object_fn = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let pc = DefaultParseCommand::new();
        let result = pc.parse_command("get thing1", &match_object_fn).unwrap();
        assert_eq!(result.verb.as_string(), "get".to_string());
        assert_eq!(result.argstr, "thing1".to_string());
        assert_eq!(result.args, vec![v_str("thing1")]);
        assert_eq!(result.dobjstr, Some("thing1".to_string()));
        assert_eq!(result.dobj, Some(MOCK_THING1));
        assert_eq!(result.prepstr, None);
        assert_eq!(result.prep, PrepSpec::None);
        assert_eq!(result.iobjstr, None);
        assert_eq!(result.iobj, None);
    }

    #[test]
    fn test_command_parser_put_thing1_in_thing2() {
        let env = setup_mock_environment();
        let match_object_fn = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = DefaultParseCommand::new()
            .parse_command("put thing1 in t2", &match_object_fn)
            .unwrap();
        assert_eq!(result.verb.as_string(), "put".to_string());
        assert_eq!(result.argstr, "thing1 in t2".to_string());
        assert_eq!(result.args, vec![v_str("thing1"), v_str("in"), v_str("t2")]);
        assert_eq!(result.dobjstr, Some("thing1".to_string()));
        assert_eq!(result.dobj, Some(MOCK_THING1));
        assert_eq!(result.prepstr, Some("in".to_string()));
        assert_eq!(result.prep, PrepSpec::Other(Preposition::IntoIn));
        assert_eq!(result.iobjstr, Some("t2".to_string()));
        assert_eq!(result.iobj, Some(MOCK_THING2));
    }

    #[test]
    fn test_command_parser_look_at_here() {
        let env = setup_mock_environment();
        let match_object_fn = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = DefaultParseCommand::new()
            .parse_command("look at here", &match_object_fn)
            .unwrap();
        assert_eq!(result.verb.as_string(), "look".to_string());
        assert_eq!(result.argstr, "at here".to_string());
        assert_eq!(result.args, vec![v_str("at"), v_str("here"),]);
        assert_eq!(result.dobjstr, None);
        assert_eq!(result.dobj, None);
        assert_eq!(result.prepstr, Some("at".to_string()));
        assert_eq!(result.prep, PrepSpec::Other(Preposition::AtTo));
        assert_eq!(result.iobjstr, Some("here".to_string()));
        assert_eq!(result.iobj, Some(MOCK_ROOM1));
    }

    #[test]
    fn test_describe_command_parsing() {
        // Test the specific command that's failing in the integration test
        let env = setup_mock_environment();
        let match_object_fn = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = DefaultParseCommand::new()
            .parse_command(
                "@describe my thing as \"A thing that's thingly.\"",
                &match_object_fn,
            )
            .unwrap();
        assert_eq!(result.verb.as_string(), "@describe".to_string());
        assert_eq!(result.dobjstr, Some("my thing".to_string()));
        assert_eq!(result.prepstr, Some("as".to_string()));
        assert_eq!(result.prep, PrepSpec::Other(Preposition::As));
        assert_eq!(result.iobjstr, Some("A thing that's thingly.".to_string()));
    }

    #[test]
    fn test_prep_confused_with_numeric_arg() {
        let env = setup_mock_environment();
        let match_object_fn = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };

        // We had a regression where the first numeric argument was being confused with a
        // preposition after I added numeric preposition support.
        let result = DefaultParseCommand::new()
            .parse_command("ins 1", &match_object_fn)
            .unwrap();
        assert_eq!(result.verb.as_string(), "ins".to_string());
        assert_eq!(result.prep, PrepSpec::None);
        assert_eq!(result.argstr, "1".to_string());
        assert_eq!(result.args, vec![v_str("1")]);
        assert_eq!(result.dobjstr, Some("1".to_string()));
        assert_eq!(result.dobj, Some(FAILED_MATCH));
        assert_eq!(result.prepstr, None);
        assert_eq!(result.iobjstr, None);
        assert_eq!(result.iobj, None);
    }
}
