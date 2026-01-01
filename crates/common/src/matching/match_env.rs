// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

use crate::{
    matching::{MatchEnvironment, MatchResult, ObjectNameMatcher},
    model::{ValSet, WorldStateError},
};
use moor_var::{AMBIGUOUS, FAILED_MATCH, Obj};

const ME: &str = "me";
const HERE: &str = "here";

#[derive(Clone, Eq, PartialEq, Debug)]
struct MatchData {
    exact: Vec<Obj>,
    partial: Vec<Obj>,
}

fn do_match_object_names(
    oid: Obj,
    match_data: &mut MatchData,
    names: Vec<String>,
    match_name: &str,
) {
    let match_name = match_name.to_lowercase();

    for object_name in names {
        let object_name = object_name.to_lowercase();
        if object_name.starts_with(&match_name.clone()) {
            // exact match
            if match_name == object_name {
                if !match_data.exact.contains(&oid) {
                    match_data.exact.push(oid);
                }
            } else {
                // partial match
                if !match_data.partial.contains(&oid) {
                    match_data.partial.push(oid);
                }
            }
        }
    }
}

fn match_contents<M: MatchEnvironment>(
    env: &M,
    player: &Obj,
    object_name: &str,
) -> Result<MatchResult, WorldStateError> {
    let mut match_data = MatchData {
        exact: Vec::new(),
        partial: Vec::new(),
    };

    let search = env.get_surroundings(player)?; // location, contents, player
    for oid in search.iter() {
        if !env.obj_valid(&oid)? {
            continue;
        }

        let object_names = env.get_names(&oid)?;
        do_match_object_names(oid, &mut match_data, object_names, object_name);
    }

    // Determine result based on exact matches first, then partial
    let (result, candidates) = if !match_data.exact.is_empty() {
        if match_data.exact.len() == 1 {
            (Some(match_data.exact[0]), Vec::new())
        } else {
            (Some(AMBIGUOUS), match_data.exact.clone())
        }
    } else if !match_data.partial.is_empty() {
        if match_data.partial.len() == 1 {
            (Some(match_data.partial[0]), Vec::new())
        } else {
            (Some(AMBIGUOUS), match_data.partial.clone())
        }
    } else {
        (Some(FAILED_MATCH), Vec::new())
    };

    Ok(MatchResult { result, candidates })
}

pub struct DefaultObjectNameMatcher<M: MatchEnvironment> {
    pub env: M,
    pub player: Obj,
}

impl<M: MatchEnvironment> ObjectNameMatcher for DefaultObjectNameMatcher<M> {
    fn match_object(&self, object_name: &str) -> Result<MatchResult, WorldStateError> {
        if object_name.is_empty() {
            return Ok(MatchResult {
                result: None,
                candidates: Vec::new(),
            });
        }

        // If it's an object number (is prefixed with # and is followed by a valid integer or UUID), return
        // an Obj directly.
        if object_name.starts_with('#')
            && let Ok(obj) = Obj::try_from(object_name)
        {
            return Ok(MatchResult {
                result: Some(obj),
                candidates: Vec::new(),
            });
        }
        // Continue with name matching if parsing fails

        // Check if the player is valid.
        if !self.env.obj_valid(&self.player)? {
            return Err(WorldStateError::FailedMatch(
                "Invalid current player when performing object match".to_string(),
            ));
        }

        // Check 'me' and 'here' first.
        if object_name == ME {
            return Ok(MatchResult {
                result: Some(self.player),
                candidates: Vec::new(),
            });
        }

        if object_name == HERE {
            return Ok(MatchResult {
                result: Some(self.env.location_of(&self.player)?),
                candidates: Vec::new(),
            });
        }

        match_contents(&self.env, &self.player, object_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::matching::{
        match_env::{
            DefaultObjectNameMatcher, MatchData, ObjectNameMatcher, do_match_object_names,
        },
        mock_matching_env::{
            MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2, setup_mock_environment,
        },
    };
    use moor_var::{NOTHING, Obj};

    #[test]
    fn test_match_object_names_fail() {
        let mut match_data = MatchData {
            exact: Vec::new(),
            partial: Vec::new(),
        };

        let names = vec!["apple", "banana", "cherry"];
        let match_name = "durian";

        do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert_eq!(
            match_data,
            MatchData {
                exact: Vec::new(),
                partial: Vec::new(),
            }
        );
    }

    #[test]
    fn test_match_object_names_exact() {
        let mut match_data = MatchData {
            exact: Vec::new(),
            partial: Vec::new(),
        };

        let names = vec!["apple", "banana", "cherry"];
        let match_name = "banana";

        do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert_eq!(
            match_data,
            MatchData {
                exact: vec![Obj::mk_id(2)],
                partial: Vec::new(),
            }
        );
    }

    #[test]
    fn test_match_object_names_partial() {
        let mut match_data = MatchData {
            exact: Vec::new(),
            partial: Vec::new(),
        };

        let names = vec!["apple", "banana", "cherry", "bunch"];
        let match_name = "b";

        do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert_eq!(
            match_data,
            MatchData {
                exact: Vec::new(),
                partial: vec![Obj::mk_id(2)],
            }
        );
    }

    #[test]
    fn test_match_object_names_ambiguous() {
        let mut match_data = MatchData {
            exact: Vec::new(),
            partial: Vec::new(),
        };

        let names = vec!["apple", "banana", "cherry", "bunch"];
        let match_name = "b";

        do_match_object_names(
            Obj::mk_id(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        // Both "banana" and "bunch" match "b", so we expect both in the list
        assert_eq!(match_data.exact.len(), 0);
        assert_eq!(match_data.partial.len(), 1);
        assert_eq!(match_data.partial[0], Obj::mk_id(2));
    }

    #[test]
    fn test_match_object_empty() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("").unwrap();
        assert_eq!(result.result, None);
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_object_number() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("#4").unwrap();
        assert_eq!(result.result, Some(MOCK_THING1));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_uuid_object() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        // Test with a UUID format object reference
        let match_result = menv.match_object("#048D05-1234567890").unwrap();
        let obj = match_result.result.unwrap();
        assert!(obj.is_uuobjid());
        assert_eq!(obj.to_literal(), "048D05-1234567890");
        assert!(match_result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_me() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("me").unwrap();
        assert_eq!(result.result, Some(MOCK_PLAYER));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_here() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("here").unwrap();
        assert_eq!(result.result, Some(MOCK_ROOM1));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_room_name() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("room1").unwrap();
        assert_eq!(result.result, Some(MOCK_ROOM1));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_room_alias() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("r1").unwrap();
        assert_eq!(result.result, Some(MOCK_ROOM1));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_player_name() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("porcupine").unwrap();
        assert_eq!(result.result, Some(MOCK_PLAYER));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_thing_name() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("thing1").unwrap();
        assert_eq!(result.result, Some(MOCK_THING1));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_thing_alias() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("t2").unwrap();
        assert_eq!(result.result, Some(MOCK_THING2));
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn test_match_object_invalid_player() {
        let env = setup_mock_environment();
        let menv = DefaultObjectNameMatcher {
            env,
            player: NOTHING,
        };
        let result = menv.match_object("thing1");
        assert!(result.is_err());
    }
}
