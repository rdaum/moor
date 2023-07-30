use crate::db::PREP_LIST;
use std::string::ToString;
use std::sync::Once;

use crate::model::r#match::PrepSpec;
use crate::var::{v_str, Objid, Var};

#[derive(Clone, Eq, PartialEq, Debug)]
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

pub const PREPOSITION_WITH_USING: u16 = 0;
pub const PREPOSITION_AT_TO: u16 = 1;
pub const PREPOSITION_IN_FRONT_OF: u16 = 2;
pub const PREPOSITION_INTO_IN: u16 = 3;
pub const PREPOSITION_ON_TOP_OF_ON: u16 = 4;
pub const PREPOSITION_OUT_OF: u16 = 5;
pub const PREPOSITION_OVER: u16 = 6;
pub const PREPOSITION_THROUGH: u16 = 7;
pub const PREPOSITION_UNDER: u16 = 8;
pub const PREPOSITION_BEHIND: u16 = 9;
pub const PREPOSITION_BESIDE: u16 = 10;
pub const PREPOSITION_FOR_ABOUT: u16 = 11;
pub const PREPOSITION_IS: u16 = 12;
pub const PREPOSITION_AS: u16 = 13;
pub const PREPOSITION_OFF_OF: u16 = 14;

static mut PREPOSITIONS: Vec<Prep> = vec![];
static INIT: Once = Once::new();

pub fn match_preposition(prep: &str) -> Option<Prep> {
    INIT.call_once(|| unsafe {
        PREPOSITIONS = PREP_LIST
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
    });
    unsafe {
        PREPOSITIONS
            .iter()
            .find(|p| p.phrases.iter().any(|t| t == &prep))
            .cloned()
    }
}

fn parse_into_words(input: &str) -> Vec<String> {
    // Initialize state variables.
    let mut in_quotes = false;
    let mut previous_char_was_backslash = false;

    // Define the fold function's logic as a closure.
    let accumulate_words = |mut acc: Vec<String>, c| {
        if previous_char_was_backslash {
            // Handle escaped characters.
            if let Some(last_word) = acc.last_mut() {
                last_word.push(c);
            } else {
                acc.push(c.to_string());
            }
            previous_char_was_backslash = false;
        } else if c == '\\' {
            // Mark the next character as escaped.
            previous_char_was_backslash = true;
        } else if c == '"' {
            // Toggle whether we're inside quotes.
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            // Add a new empty string to the accumulator if we've reached a whitespace boundary.
            if let Some(last_word) = acc.last() {
                if !last_word.is_empty() {
                    acc.push(String::new());
                }
            }
        } else {
            // Append the current character to the last word in the accumulator,
            // or create a new word if there isn't one yet.
            if let Some(last_word) = acc.last_mut() {
                last_word.push(c);
            } else {
                acc.push(c.to_string());
            }
        }
        acc
    };

    // Use the fold function to accumulate the words in the input string.
    let words = input.chars().fold(vec![], accumulate_words);

    // Filter out empty strings and return the result.
    words.into_iter().filter(|w| !w.is_empty()).collect()
}

pub fn parse_command<F>(input: &str, mut match_object_fn: F) -> ParsedCommand
where
    F: FnMut(&str) -> Option<Objid>,
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
    let words = parse_into_words(&command);

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
        // TODO: Handle built-in commands
        unimplemented!("Built-in commands not implemented");
    }
    // Split into verb and argument string
    let mut parts = command.splitn(2, ' ');
    let verb = parts.next().unwrap_or_default().to_string();
    let argstr = parts.next().unwrap_or_default().to_string();

    let words = parse_into_words(&argstr);

    // Normal MOO command
    // Find preposition, if any
    let mut prep_index = None;
    for (j, word) in words.iter().enumerate() {
        if let Some(p) = match_preposition(word) {
            prep_index = Some(j);
            prepstr = word.to_string();
            prep = PrepSpec::Other(p.id as u16);
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
        iobj = match_object_fn(&iobjstr);
    }

    // Get direct object object
    if !dobjstr.is_empty() {
        dobj = match_object_fn(&dobjstr);
    }

    // Build and return ParsedCommand
    let args: Vec<Var> = words.iter().map(|w| v_str(w)).collect();

    ParsedCommand {
        verb,
        argstr,
        args,
        dobjstr,
        dobj: dobj.unwrap_or(Objid(-1)),
        prepstr,
        prep,
        iobjstr,
        iobj: iobj.unwrap_or(Objid(-1)),
    }
}

#[cfg(test)]
mod tests {
    use crate::db::matching::world_environment_match_object;
    use crate::db::mock_matching_env::{
        setup_mock_environment, MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2,
    };
    use crate::var::{v_str, NOTHING};

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
        let input = r#"hello\ world frankly"#;
        let expected_output = vec!["hello world", "frankly"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    #[test]
    fn test_parse_into_words_mixed() {
        let input = r#"hello "big world"\" foo bar\"baz"#;
        let expected_output = vec!["hello", "big world\"", "foo", "bar\"baz"];
        assert_eq!(parse_into_words(input), expected_output);
    }

    fn simple_match_object(objstr: &str) -> Option<Objid> {
        match objstr {
            "obj" => Some(Objid(1)),
            "player" => Some(Objid(2)),
            _ => None,
        }
    }

    #[test]
    fn test_parse_single_arg_command() {
        // Test normal MOO command
        let command = "look obj";
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
        assert_eq!(parsed.verb, "give");
        assert_eq!(parsed.dobjstr, "obj");
        assert_eq!(parsed.dobj, Objid(1));
        assert_eq!(parsed.prepstr, "to");
        assert_eq!(parsed.prep, PrepSpec::Other(PREPOSITION_AT_TO));
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let parsed = parse_command(command, simple_match_object);
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
        let mut env = setup_mock_environment();
        let match_object_fn =
            |name: &str| world_environment_match_object(&mut env, MOCK_PLAYER, name).unwrap();

        let result = parse_command("get thing1", match_object_fn);
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
        let mut env = setup_mock_environment();
        let match_object_fn =
            |name: &str| world_environment_match_object(&mut env, MOCK_PLAYER, name).unwrap();

        let result = parse_command("put thing1 in t2", match_object_fn);
        assert_eq!(result.verb, "put".to_string());
        assert_eq!(result.argstr, "thing1 in t2".to_string());
        assert_eq!(result.args, vec![v_str("thing1"), v_str("in"), v_str("t2")]);
        assert_eq!(result.dobjstr, "thing1".to_string());
        assert_eq!(result.dobj, MOCK_THING1);
        assert_eq!(result.prepstr, "in".to_string());
        assert_eq!(result.prep, PrepSpec::Other(PREPOSITION_INTO_IN));
        assert_eq!(result.iobjstr, "t2".to_string());
        assert_eq!(result.iobj, MOCK_THING2);
    }

    #[test]
    fn test_command_parser_look_at_here() {
        let mut env = setup_mock_environment();
        let match_object_fn =
            |name: &str| world_environment_match_object(&mut env, MOCK_PLAYER, name).unwrap();

        let result = parse_command("look at here", match_object_fn);
        assert_eq!(result.verb, "look".to_string());
        assert_eq!(result.argstr, "at here".to_string());
        assert_eq!(result.args, vec![v_str("at"), v_str("here"),]);
        assert_eq!(result.dobjstr, "".to_string());
        assert_eq!(result.dobj, NOTHING);
        assert_eq!(result.prepstr, "at".to_string());
        assert_eq!(result.prep, PrepSpec::Other(PREPOSITION_AT_TO));
        assert_eq!(result.iobjstr, "here".to_string());
        assert_eq!(result.iobj, MOCK_ROOM1);
    }
}
