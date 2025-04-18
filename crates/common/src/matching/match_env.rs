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

use crate::matching::command_parse::ParseMatcher;
use crate::model::WorldStateError;
use crate::model::{ObjSet, ValSet};
use moor_var::Obj;
use moor_var::{AMBIGUOUS, FAILED_MATCH, NOTHING};

// This is the interface that the matching code needs to be able to call into the world state.
// Separated out so can be more easily mocked.
pub trait MatchEnvironment {
    // Test whether a given object is valid in this environment.
    fn obj_valid(&self, oid: &Obj) -> Result<bool, WorldStateError>;

    // Return all match names & aliases for an object.
    fn get_names(&self, oid: &Obj) -> Result<Vec<String>, WorldStateError>;

    // Returns location, contents, and player, all the things we'd search for matches on.
    fn get_surroundings(&self, player: &Obj) -> Result<ObjSet, WorldStateError>;

    // Return the location of a given object.
    fn location_of(&self, player: &Obj) -> Result<Obj, WorldStateError>;
}

#[derive(Clone, Eq, PartialEq, Debug)]
struct MatchData {
    exact: Obj,
    partial: Obj,
}

fn do_match_object_names(
    oid: Obj,
    match_data: &mut MatchData,
    names: Vec<String>,
    match_name: &str,
) -> Result<Obj, WorldStateError> {
    let match_name = match_name.to_lowercase();

    for object_name in names {
        let object_name = object_name.to_lowercase();
        if object_name.starts_with(&match_name.clone()) {
            // exact match
            if match_name == object_name {
                if match_data.exact == NOTHING || match_data.exact == oid {
                    match_data.exact = oid.clone();
                } else {
                    return Ok(AMBIGUOUS);
                }
            } else {
                // partial match
                if match_data.partial == FAILED_MATCH || match_data.partial == oid {
                    match_data.partial = oid.clone();
                } else {
                    match_data.partial = AMBIGUOUS
                }
            }
        }
    }

    if match_data.exact != NOTHING {
        Ok(match_data.exact.clone())
    } else {
        Ok(match_data.partial.clone())
    }
}

pub fn match_contents<M: MatchEnvironment>(
    env: &M,
    player: &Obj,
    object_name: &str,
) -> Result<Option<Obj>, WorldStateError> {
    let mut match_data = MatchData {
        exact: NOTHING,
        partial: FAILED_MATCH,
    };

    let search = env.get_surroundings(player)?; // location, contents, player
    for oid in search.iter() {
        if !env.obj_valid(&oid)? {
            continue;
        }

        let object_names = env.get_names(&oid)?;
        let result = do_match_object_names(oid, &mut match_data, object_names, object_name)?;
        if result == AMBIGUOUS {
            return Ok(Some(AMBIGUOUS));
        }
    }
    if match_data.exact != NOTHING {
        Ok(Some(match_data.exact))
    } else {
        Ok(Some(match_data.partial))
    }
}

pub struct MatchEnvironmentParseMatcher<M: MatchEnvironment> {
    pub env: M,
    pub player: Obj,
}

impl<M: MatchEnvironment> ParseMatcher for MatchEnvironmentParseMatcher<M> {
    fn match_object(&self, object_name: &str) -> Result<Option<Obj>, WorldStateError> {
        if object_name.is_empty() {
            return Ok(None);
        }

        // If if's an object number (is prefixed with # and is followed by a valid integer), return
        // an Objid directly.
        if let Some(stripped) = object_name.strip_prefix('#') {
            let object_number = stripped.parse::<i32>();
            if let Ok(object_number) = object_number {
                return Ok(Some(Obj::mk_id(object_number)));
            }
        }

        // Check if the player is valid.
        if !self.env.obj_valid(&self.player)? {
            return Err(WorldStateError::FailedMatch(
                "Invalid current player when performing object match".to_string(),
            ));
        }

        // Check 'me' and 'here' first.
        if object_name == "me" {
            return Ok(Some(self.player.clone()));
        }

        if object_name == "here" {
            return Ok(Some(self.env.location_of(&self.player)?));
        }

        match_contents(&self.env, &self.player, object_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::matching::command_parse::ParseMatcher;
    use crate::matching::match_env::{
        MatchData, MatchEnvironmentParseMatcher, do_match_object_names,
    };
    use crate::matching::mock_matching_env::{
        MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2, setup_mock_environment,
    };
    use moor_var::Obj;
    use moor_var::{FAILED_MATCH, NOTHING};

    #[test]
    fn test_match_object_names_fail() {
        let mut match_data = MatchData {
            exact: NOTHING,
            partial: FAILED_MATCH,
        };

        let names = vec!["apple", "banana", "cherry"];
        let match_name = "durian";

        let result = do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), FAILED_MATCH);
        assert_eq!(
            match_data,
            MatchData {
                exact: NOTHING,
                partial: FAILED_MATCH,
            }
        );
    }

    #[test]
    fn test_match_object_names_exact() {
        let mut match_data = MatchData {
            exact: NOTHING,
            partial: FAILED_MATCH,
        };

        let names = vec!["apple", "banana", "cherry"];
        let match_name = "banana";

        let result = do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Obj::mk_id(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: Obj::mk_id(2),
                partial: FAILED_MATCH,
            }
        );
    }

    #[test]
    fn test_match_object_names_partial() {
        let mut match_data = MatchData {
            exact: NOTHING,
            partial: FAILED_MATCH,
        };

        let names = vec!["apple", "banana", "cherry", "bunch"];
        let match_name = "b";

        let result = do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Obj::mk_id(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: NOTHING,
                partial: Obj::mk_id(2),
            }
        );
    }

    #[test]
    fn test_match_object_names_ambiguous() {
        let mut match_data = MatchData {
            exact: NOTHING,
            partial: FAILED_MATCH,
        };

        let names = vec!["apple", "banana", "cherry", "bunch"];
        let match_name = "b";

        let result = do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Obj::mk_id(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: NOTHING,
                partial: Obj::mk_id(2),
            }
        );
    }

    #[test]
    fn test_match_object_empty() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("");
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_match_object_object_number() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("#4");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_match_object_me() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("me");
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[test]
    fn test_match_object_here() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("here");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_room_name() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("room1");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_room_alias() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("r1");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_player_name() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("porcupine");
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[test]
    fn test_match_object_thing_name() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("thing1");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_match_object_thing_alias() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("t2");
        assert_eq!(result.unwrap(), Some(MOCK_THING2));
    }

    #[test]
    fn test_match_object_invalid_player() {
        let env = setup_mock_environment();
        let menv = MatchEnvironmentParseMatcher {
            env,
            player: NOTHING,
        };
        let result = menv.match_object("thing1");
        assert!(result.is_err());
    }
}
