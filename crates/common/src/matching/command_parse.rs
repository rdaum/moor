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

use std::string::ToString;

use bincode::{Decode, Encode};

use crate::model::WorldStateError;
use crate::model::{PrepSpec, Preposition};
use crate::util;
use moor_var::{Obj, Symbol};
use moor_var::{Var, v_str};

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub struct ParsedCommand {
    pub verb: Symbol,
    pub argstr: String,
    pub args: Vec<Var>,

    pub dobjstr: Option<String>,
    pub dobj: Option<Obj>,

    pub prepstr: Option<String>,
    pub prep: PrepSpec,

    pub iobjstr: Option<String>,
    pub iobj: Option<Obj>,
}

/// Match a preposition for the form used by set_verb_args and friends, which means it must support
/// numeric arguments for the preposition.
pub fn parse_preposition_spec(repr: &str) -> Option<PrepSpec> {
    match repr {
        "any" => Some(PrepSpec::Any),
        "none" => Some(PrepSpec::None),
        _ => find_preposition(repr).map(PrepSpec::Other),
    }
}

pub fn find_preposition(prep: &str) -> Option<Preposition> {
    // If the string starts with a number (with or without # prefix), treat it as a preposition ID.
    let numeric_offset = if prep.starts_with('#') { 1 } else { 0 };
    if let Ok(id) = prep[numeric_offset..].parse::<u16>() {
        return Preposition::from_repr(id);
    }

    Preposition::parse(prep)
}

pub fn preposition_to_string(ps: &PrepSpec) -> &str {
    match ps {
        PrepSpec::Any => "any",
        PrepSpec::None => "none",
        PrepSpec::Other(id) => id.to_string(),
    }
}

// Seeks the first preposition in a list of words, returning the index of the preposition, the
// preposition itself, and the preposition string, if any.
fn seek_preposition(words: &[String]) -> (Option<(usize, String)>, PrepSpec) {
    for (j, word) in words.iter().enumerate() {
        if Preposition::parse(word.as_str()).is_some() {
            return (
                Some((j, word.clone())),
                PrepSpec::Other(Preposition::parse(word).unwrap()),
            );
        }
    }
    (None, PrepSpec::None)
}

pub trait ParseMatcher {
    fn match_object(&self, name: &str) -> Result<Option<Obj>, WorldStateError>;
}

#[derive(thiserror::Error, Debug, Clone, Decode, Encode)]
pub enum ParseCommandError {
    #[error("Empty command")]
    EmptyCommand,
    #[error("Unimplemented built-in command")]
    UnimplementedBuiltInCommand,
    #[error("Error occurred during object matching")]
    ErrorDuringMatch(WorldStateError),
    #[error("Permission denied")]
    PermissionDenied,
}

pub fn parse_command<M>(
    input: &str,
    command_environment: M,
) -> Result<ParsedCommand, ParseCommandError>
where
    M: ParseMatcher,
{
    // Replace initial command characters with say/emote/eval
    let mut command = input.trim_start().to_string();
    let first_char = command.chars().next().unwrap_or(' ');
    match first_char {
        '"' => command.replace_range(..1, "say "),
        ':' => command.replace_range(..1, "emote "),
        ';' => command.replace_range(..1, "eval "),
        _ => {}
    };

    // Get word list
    let words = util::parse_into_words(&command);
    if words.is_empty() {
        return Err(ParseCommandError::EmptyCommand);
    }

    // Split into verb and argument string
    let mut parts = command.splitn(2, ' ');
    let verb = parts.next().unwrap_or_default().into();
    let argstr = parts.next().unwrap_or_default().to_string();

    let words = util::parse_into_words(&argstr);

    // Normal MOO command
    // Find preposition, if any

    let (prep_match, prep) = seek_preposition(&words);

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
    let dobj = match &dobjstr {
        Some(dobjstr) => command_environment
            .match_object(dobjstr)
            .map_err(ParseCommandError::ErrorDuringMatch)?,
        None => None,
    };

    // Get indirect object string
    let iobjstr = prep_match.as_ref().map(|(j, _)| words[j + 1..].join(" "));

    // Get indirect object object
    let iobj = match (prep, &iobjstr) {
        (PrepSpec::None, _) => None,
        (_, Some(iobjstr)) => command_environment
            .match_object(iobjstr)
            .map_err(ParseCommandError::ErrorDuringMatch)?,
        _ => None,
    };

    // Build and return ParsedCommand
    let args: Vec<Var> = words.iter().map(|w| v_str(w)).collect();

    Ok(ParsedCommand {
        verb,
        argstr,
        args,
        dobjstr,
        dobj,
        prepstr: prep_match.map(|(_, p)| p.to_string()),
        prep,
        iobjstr,
        iobj,
    })
}

#[cfg(test)]
mod tests {
    use crate::model::Preposition;
    use crate::util::parse_into_words;
    use moor_var::FAILED_MATCH;
    use moor_var::v_str;

    use crate::matching::match_env::MatchEnvironmentParseMatcher;
    use crate::matching::mock_matching_env::{
        MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2, setup_mock_environment,
    };

    use super::*;

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

    struct SimpleParseMatcher {}
    impl ParseMatcher for SimpleParseMatcher {
        fn match_object(&self, name: &str) -> Result<Option<Obj>, WorldStateError> {
            Ok(match name {
                "obj" => Some(Obj::mk_id(1)),
                "player" => Some(Obj::mk_id(2)),
                _ => None,
            })
        }
    }

    #[test]
    fn test_parse_single_arg_command() {
        // Test normal MOO command
        let command = "look obj";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "look");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();

        assert_eq!(parsed.verb.as_str(), "test");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "give");
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
    fn test_parse_say_abbrev_command() {
        // Test say abbrev command
        let command = "\"hello, world!";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "say");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "emote");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "emote");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "eval");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "blork");
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
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb.as_str(), "say");
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
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = parse_command("get thing1", match_object_fn).unwrap();
        assert_eq!(result.verb.as_str(), "get".to_string());
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
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = parse_command("put thing1 in t2", match_object_fn).unwrap();
        assert_eq!(result.verb.as_str(), "put".to_string());
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
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = parse_command("look at here", match_object_fn).unwrap();
        assert_eq!(result.verb.as_str(), "look".to_string());
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
    fn test_prep_confused_with_numeric_arg() {
        let env = setup_mock_environment();
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };

        // We had a regression where the first numeric argument was being confused with a
        // preposition after I added numeric preposition support.
        let result = parse_command("ins 1", match_object_fn).unwrap();
        assert_eq!(result.verb.as_str(), "ins".to_string());
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
