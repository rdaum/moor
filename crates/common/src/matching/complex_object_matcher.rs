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

//! Enhanced object name matcher using complex_match functionality

use crate::matching::{
    ComplexMatchResult, MatchEnvironment, ObjectNameMatcher, complex_match_objects_keys,
};
use crate::model::{ValSet, WorldStateError};
use moor_var::{AMBIGUOUS, FAILED_MATCH, Obj, Var, v_list, v_str};

const ME: &str = "me";
const HERE: &str = "here";

/// Enhanced object name matcher that uses complex_match for sophisticated object resolution
/// with ordinal support and three-tier matching (exact, prefix, substring)
pub struct ComplexObjectNameMatcher<M: MatchEnvironment> {
    pub env: M,
    pub player: Obj,
}

impl<M: MatchEnvironment> ObjectNameMatcher for ComplexObjectNameMatcher<M> {
    fn match_object(&self, object_name: &str) -> Result<Option<Obj>, WorldStateError> {
        if object_name.is_empty() {
            return Ok(None);
        }

        // Handle object number references (e.g. "#123" or UUID format)
        if object_name.starts_with('#')
            && let Ok(obj) = Obj::try_from(object_name)
        {
            return Ok(Some(obj));
        }
        // Continue with name matching if parsing fails

        // Check if the player is valid
        if !self.env.obj_valid(&self.player)? {
            return Err(WorldStateError::FailedMatch(
                "Invalid current player when performing object match".to_string(),
            ));
        }

        // Handle special keywords
        if object_name.eq_ignore_ascii_case(ME) {
            return Ok(Some(self.player));
        }

        if object_name.eq_ignore_ascii_case(HERE) {
            return Ok(Some(self.env.location_of(&self.player)?));
        }

        // Get objects in the environment (location, contents, player)
        let search_objects = self.env.get_surroundings(&self.player)?;
        let mut objects = Vec::new();
        let mut names_lists = Vec::new();

        // Collect valid objects and their names
        for oid in search_objects.iter() {
            if !self.env.obj_valid(&oid)? {
                continue;
            }

            let object_names = self.env.get_names(&oid)?;
            if object_names.is_empty() {
                continue;
            }

            objects.push(Var::from(oid));
            // Convert names to list of string Vars
            let name_vars: Vec<Var> = object_names.iter().map(|name| v_str(name)).collect();
            names_lists.push(v_list(&name_vars));
        }

        if objects.is_empty() {
            return Ok(Some(FAILED_MATCH));
        }

        // Use complex_match to find the best match
        match complex_match_objects_keys(object_name, &objects, &names_lists) {
            ComplexMatchResult::NoMatch => Ok(Some(FAILED_MATCH)),
            ComplexMatchResult::Single(obj_var) => {
                // Convert Var back to Obj
                let moor_var::Variant::Obj(obj) = obj_var.variant() else {
                    return Ok(Some(FAILED_MATCH));
                };
                Ok(Some(*obj))
            }
            ComplexMatchResult::Multiple(matches) => {
                // Multiple matches - this is ambiguous in MOO
                if matches.len() <= 1 {
                    let Some(first_match) = matches.first() else {
                        return Ok(Some(FAILED_MATCH));
                    };
                    let moor_var::Variant::Obj(obj) = first_match.variant() else {
                        return Ok(Some(FAILED_MATCH));
                    };
                    Ok(Some(*obj))
                } else {
                    Ok(Some(AMBIGUOUS))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching::mock_matching_env::{
        MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2, setup_mock_environment,
    };

    #[test]
    fn test_complex_match_object_empty() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("");
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_complex_match_object_number() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("#4");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_complex_match_uuid_object() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        // Test with a UUID format object reference
        let result = matcher.match_object("#048D05-1234567890");
        assert!(result.is_ok());
        let obj = result.unwrap().unwrap();
        assert!(obj.is_uuobjid());
        assert_eq!(obj.to_literal(), "048D05-1234567890");
    }

    #[test]
    fn test_complex_match_me() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("me");
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[test]
    fn test_complex_match_here() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("here");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_complex_match_exact_object_name() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("thing1");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_complex_match_object_alias() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("t2");
        assert_eq!(result.unwrap(), Some(MOCK_THING2));
    }

    #[test]
    fn test_complex_match_with_ordinal() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        // This should find the first object matching "t" (partial match)
        // In the mock environment, both thing1 and thing2 have aliases starting with "t"
        let result = matcher.match_object("first t");
        // Should return one of the things (the first match)
        assert!(result.is_ok());
        let obj = result.unwrap();
        assert!(obj == Some(MOCK_THING1) || obj == Some(MOCK_THING2));
    }

    #[test]
    fn test_complex_match_substring() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        // "hing" should match "thing1" and "thing2" as substring
        let result = matcher.match_object("hing");
        assert!(result.is_ok());
        // Should be ambiguous or return one of them
        let obj = result.unwrap();
        assert!(obj == Some(MOCK_THING1) || obj == Some(MOCK_THING2) || obj == Some(AMBIGUOUS));
    }

    #[test]
    fn test_complex_match_no_match() {
        let env = setup_mock_environment();
        let matcher = ComplexObjectNameMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = matcher.match_object("nonexistent");
        assert_eq!(result.unwrap(), Some(FAILED_MATCH));
    }
}
