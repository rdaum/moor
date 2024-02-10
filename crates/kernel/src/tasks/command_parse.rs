// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
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
use lazy_static::lazy_static;

use moor_values::model::WorldStateError;
use moor_values::model::{PrepSpec, Preposition, PREP_LIST};
use moor_values::util;
use moor_values::var::Objid;
use moor_values::var::{v_str, Var};

lazy_static! {
    static ref PREPOSITIONS: Vec<Prep> = {
        PREP_LIST
            .iter()
            .enumerate()
            .map(|(id, phrase)| {
                let phrases = phrase
                    .split('/')
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<&str>>();
                Prep { id, phrases }
            })
            .collect::<Vec<Prep>>()
    };
}

#[derive(Clone, Eq, PartialEq, Debug, Decode, Encode)]
pub struct ParsedCommand {
    pub verb: String,
    pub argstr: String,
    pub args: Vec<Var>,

    pub dobjstr: String,
    pub dobj: Objid,

    pub prepstr: String,
    pub prep: PrepSpec,

    pub iobjstr: String,
    pub iobj: Objid,
}

#[derive(Clone)]
pub struct Prep {
    pub id: usize,
    phrases: Vec<&'static str>,
}

/// Match a preposition for the form used by set_verb_args and friends, which means it must support
/// numeric arguments for the preposition.
pub fn parse_preposition_spec(repr: &str) -> Option<PrepSpec> {
    match repr {
        "any" => Some(PrepSpec::Any),
        "none" => Some(PrepSpec::None),
        _ => find_preposition(repr)
            .map(|p| PrepSpec::Other(Preposition::from_repr(p.id as u16).unwrap())),
    }
}

pub fn find_preposition(prep: &str) -> Option<Prep> {
    // If the string starts with a number (with or without # prefix), treat it as a preposition ID.
    // Which is the offset into the PREPOSITIONS array.
    if let Some(id) = prep.strip_prefix('#') {
        if let Ok(id) = id.parse::<usize>() {
            return PREPOSITIONS.get(id).cloned();
        }
    } else if let Ok(id) = prep.parse::<usize>() {
        return PREPOSITIONS.get(id).cloned();
    }

    match_preposition(prep)
}

pub fn preposition_to_string(ps: &PrepSpec) -> &str {
    match ps {
        PrepSpec::Any => "any",
        PrepSpec::None => "none",
        PrepSpec::Other(id) => PREP_LIST[*id as usize],
    }
}

/// Match a preposition for the form used by the parser, where using a number is not correct.
fn match_preposition(prep: &str) -> Option<Prep> {
    // Otherwise, search for the preposition in the list of prepositions by string.
    PREPOSITIONS
        .iter()
        .find(|p| p.phrases.iter().any(|t| t == &prep))
        .cloned()
}

pub trait ParseMatcher {
    fn match_object(&mut self, name: &str) -> Result<Option<Objid>, WorldStateError>;
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

#[tracing::instrument(skip(command_environment))]
pub fn parse_command<M>(
    input: &str,
    mut command_environment: M,
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

    // Check for built-in commands
    let i = 0;
    let verb = words[i].to_string();
    let dobjstr;
    let mut dobj = None;
    let mut prepstr = String::new();
    let mut prep = PrepSpec::None;
    let mut iobjstr = String::new();
    let mut iobj = None;
    if [
        "PREFIX",
        "OUTPUTPREFIX",
        "SUFFIX",
        "OUTPUTSUFFIX",
        ".program",
        ".flush",
    ]
    .contains(&verb.as_str())
    {
        // TODO: Handle built-in commands like .program, .flush, etc.
        return Err(ParseCommandError::UnimplementedBuiltInCommand);
    }
    // Split into verb and argument string
    let mut parts = command.splitn(2, ' ');
    let verb = parts.next().unwrap_or_default().to_string();
    let argstr = parts.next().unwrap_or_default().to_string();

    let words = util::parse_into_words(&argstr);

    // Normal MOO command
    // Find preposition, if any
    let mut prep_index = None;
    for (j, word) in words.iter().enumerate() {
        if let Some(p) = match_preposition(word) {
            prep_index = Some(j);
            prepstr = word.to_string();
            prep = PrepSpec::Other(Preposition::from_repr(p.id as u16).unwrap());
            break;
        }
    }

    // Get direct object string
    if let Some(j) = prep_index {
        dobjstr = words[0..j].join(" ");
    } else {
        dobjstr = words.join(" ");
    }

    // Get indirect object string
    if let Some(j) = prep_index {
        iobjstr = words[j + 1..].join(" ");
    }

    // Get indirect object object
    if prep != PrepSpec::None && !iobjstr.is_empty() {
        iobj = command_environment
            .match_object(&iobjstr)
            .map_err(ParseCommandError::ErrorDuringMatch)?;
    }

    // Get direct object object
    if !dobjstr.is_empty() {
        dobj = command_environment
            .match_object(&dobjstr)
            .map_err(ParseCommandError::ErrorDuringMatch)?;
    }

    // Build and return ParsedCommand
    let args: Vec<Var> = words.iter().map(|w| v_str(w)).collect();

    Ok(ParsedCommand {
        verb,
        argstr,
        args,
        dobjstr,
        dobj: dobj.unwrap_or(Objid(-1)),
        prepstr,
        prep,
        iobjstr,
        iobj: iobj.unwrap_or(Objid(-1)),
    })
}

#[cfg(test)]
mod tests {
    use moor_values::model::Preposition;
    use moor_values::util::parse_into_words;
    use moor_values::var::v_str;
    use moor_values::{FAILED_MATCH, NOTHING};

    use crate::matching::match_env::MatchEnvironmentParseMatcher;
    use crate::matching::mock_matching_env::{
        setup_mock_environment, MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2,
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
        fn match_object(&mut self, name: &str) -> Result<Option<Objid>, WorldStateError> {
            Ok(match name {
                "obj" => Some(Objid(1)),
                "player" => Some(Objid(2)),
                _ => None,
            })
        }
    }

    #[test]
    fn test_parse_single_arg_command() {
        // Test normal MOO command
        let command = "look obj";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "look");
        assert_eq!(parsed.dobjstr, "obj");
        assert_eq!(parsed.dobj, Objid(1));
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.iobj, Objid(-1));
        assert_eq!(parsed.args, vec![v_str("obj")]);
        assert_eq!(parsed.argstr, "obj");
    }

    #[test]
    fn test_parse_multi_arg_command() {
        // Test normal MOO command with multiple args
        let command = "test arg1 arg2 arg3";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "test");
        assert_eq!(parsed.dobjstr, "arg1 arg2 arg3");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
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
        assert_eq!(parsed.verb, "give");
        assert_eq!(parsed.dobjstr, "obj");
        assert_eq!(parsed.dobj, Objid(1));
        assert_eq!(parsed.prepstr, "to");
        assert_eq!(parsed.prep, PrepSpec::Other(Preposition::AtTo));
        assert_eq!(parsed.iobjstr, "player");
        assert_eq!(parsed.iobj, Objid(2));
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
        assert_eq!(parsed.verb, "say");
        assert_eq!(parsed.dobjstr, "hello, world!");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("hello,"), v_str("world!")]);
        assert_eq!(parsed.argstr, "hello, world!");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_parse_emote_command() {
        // Test emote command
        let command = ":waves happily.";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "emote");
        assert_eq!(parsed.dobjstr, "waves happily.");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("waves"), v_str("happily.")]);
        assert_eq!(parsed.argstr, "waves happily.");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_parse_emote_explicit_command() {
        // Test emote command
        let command = "emote waves happily.";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "emote");
        assert_eq!(parsed.dobjstr, "waves happily.");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("waves"), v_str("happily.")]);
        assert_eq!(parsed.argstr, "waves happily.");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_parse_eval_command() {
        // Test eval command
        let command = ";1 + 1";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "eval");
        assert_eq!(parsed.dobjstr, "1 + 1");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("1"), v_str("+"), v_str("1")]);
        assert_eq!(parsed.argstr, "1 + 1");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_parse_quoted_arg_command() {
        // Test command with single escaped argument
        let command = "blork \"hello, world!\"";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "blork");
        assert_eq!(parsed.dobjstr, "hello, world!");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("hello, world!")]);
        assert_eq!(parsed.argstr, "\"hello, world!\"");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_parse_say_abbrev_quoted_command() {
        // Test say abbrev command
        let command = "\"\"hello, world!\"";
        let parsed = parse_command(command, SimpleParseMatcher {}).unwrap();
        assert_eq!(parsed.verb, "say");
        assert_eq!(parsed.dobjstr, "hello, world!");
        assert_eq!(parsed.prepstr, "");
        assert_eq!(parsed.iobjstr, "");
        assert_eq!(parsed.args, vec![v_str("hello, world!")]);
        assert_eq!(parsed.argstr, "\"hello, world!\"");
        assert_eq!(parsed.dobj, Objid(-1));
        assert_eq!(parsed.iobj, Objid(-1));
    }

    #[test]
    fn test_command_parser_get_thing1() {
        let env = setup_mock_environment();
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = parse_command("get thing1", match_object_fn).unwrap();
        assert_eq!(result.verb, "get".to_string());
        assert_eq!(result.argstr, "thing1".to_string());
        assert_eq!(result.args, vec![v_str("thing1")]);
        assert_eq!(result.dobjstr, "thing1".to_string());
        assert_eq!(result.dobj, MOCK_THING1);
        assert_eq!(result.prepstr, "".to_string());
        assert_eq!(result.prep, PrepSpec::None);
        assert_eq!(result.iobjstr, "".to_string());
        assert_eq!(result.iobj, NOTHING);
    }

    #[test]
    fn test_command_parser_put_thing1_in_thing2() {
        let env = setup_mock_environment();
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = parse_command("put thing1 in t2", match_object_fn).unwrap();
        assert_eq!(result.verb, "put".to_string());
        assert_eq!(result.argstr, "thing1 in t2".to_string());
        assert_eq!(result.args, vec![v_str("thing1"), v_str("in"), v_str("t2")]);
        assert_eq!(result.dobjstr, "thing1".to_string());
        assert_eq!(result.dobj, MOCK_THING1);
        assert_eq!(result.prepstr, "in".to_string());
        assert_eq!(result.prep, PrepSpec::Other(Preposition::IntoIn));
        assert_eq!(result.iobjstr, "t2".to_string());
        assert_eq!(result.iobj, MOCK_THING2);
    }

    #[test]
    fn test_command_parser_look_at_here() {
        let env = setup_mock_environment();
        let match_object_fn = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };

        let result = parse_command("look at here", match_object_fn).unwrap();
        assert_eq!(result.verb, "look".to_string());
        assert_eq!(result.argstr, "at here".to_string());
        assert_eq!(result.args, vec![v_str("at"), v_str("here"),]);
        assert_eq!(result.dobjstr, "".to_string());
        assert_eq!(result.dobj, NOTHING);
        assert_eq!(result.prepstr, "at".to_string());
        assert_eq!(result.prep, PrepSpec::Other(Preposition::AtTo));
        assert_eq!(result.iobjstr, "here".to_string());
        assert_eq!(result.iobj, MOCK_ROOM1);
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

        assert_eq!(result.verb, "ins".to_string());
        assert_eq!(result.prep, PrepSpec::None);
        assert_eq!(result.argstr, "1".to_string());
        assert_eq!(result.args, vec![v_str("1")]);
        assert_eq!(result.dobjstr, "1".to_string());
        assert_eq!(result.dobj, FAILED_MATCH);
        assert_eq!(result.prepstr, "".to_string());
        assert_eq!(result.iobjstr, "".to_string());
        assert_eq!(result.iobj, NOTHING);
    }
}
