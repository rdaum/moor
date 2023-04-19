use anyhow::anyhow;

use crate::db::state::StateError;
use crate::model::var::{Objid, AMBIGUOUS, FAILED_MATCH, NOTHING};

// This is the interface that the matching code needs to be able to call into the world state.
// Separated out so can be more easily mocked.
pub trait MatchEnvironment {
    // Test whether a given object is valid in this environment.
    fn obj_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error>;

    // Return all match names & aliases for an object.
    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error>;

    // Returns location, contents, and player, all the things we'd search for matches on.
    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, anyhow::Error>;

    // Return the location of a given object.
    fn location_of(&mut self, player: Objid) -> Result<Objid, anyhow::Error>;
}

#[derive(Clone, Eq, PartialEq, Debug)]
struct MatchData {
    exact: Objid,
    partial: Objid,
}

fn do_match_object_names(
    oid: Objid,
    match_data: &mut MatchData,
    names: Vec<String>,
    match_name: &str,
) -> Result<Objid, anyhow::Error> {
    let match_name = match_name.to_lowercase();

    for object_name in names {
        let object_name = object_name.to_lowercase();
        if object_name.starts_with(&match_name.clone()) {
            // exact match
            if match_name == object_name {
                if match_data.exact == NOTHING || match_data.exact == oid {
                    match_data.exact = oid;
                } else {
                    return Ok(AMBIGUOUS);
                }
            } else {
                // partial match
                if match_data.partial == FAILED_MATCH || match_data.partial == oid {
                    match_data.partial = oid;
                } else {
                    match_data.partial = AMBIGUOUS
                }
            }
        }
    }

    if match_data.exact != NOTHING {
        Ok(match_data.exact)
    } else {
        Ok(match_data.partial)
    }
}

pub fn match_contents(
    env: &mut dyn MatchEnvironment,
    player: Objid,
    object_name: &str,
) -> Result<Option<Objid>, anyhow::Error> {
    let mut match_data = MatchData {
        exact: NOTHING,
        partial: FAILED_MATCH,
    };

    let search = env.get_surroundings(player)?; // location, contents, player
    for oid in search {
        if !env.obj_valid(oid)? {
            continue;
        }

        let object_names = env.get_names(oid)?;
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

pub fn world_environment_match_object(
    env: &mut dyn MatchEnvironment,
    player: Objid,
    object_name: &str,
) -> Result<Option<Objid>, anyhow::Error> {
    if object_name.is_empty() {
        return Ok(None);
    }

    // If if's an object number (is prefixed with # and is followed by a valid integer), return
    // an Objid directly.
    if let Some(stripped) = object_name.strip_prefix('#') {
        let object_number = stripped.parse::<i64>();
        if let Ok(object_number) = object_number {
            return Ok(Some(Objid(object_number)));
        }
    }

    // Check if the player is valid.
    if !env.obj_valid(player)? {
        return Err(anyhow!(StateError::FailedMatch(
            "Invalid current player when performing object match".to_string()
        )));
    }

    // Check 'me' and 'here' first.
    if object_name == "me" {
        return Ok(Some(player));
    }

    if object_name == "here" {
        return Ok(Some(env.location_of(player)?));
    }

    match_contents(env, player, object_name)
}

#[cfg(test)]
mod tests {
    use crate::db::matching::{do_match_object_names, world_environment_match_object, MatchData};
    use crate::db::mock_matching_env::{
        setup_mock_environment, MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2,
    };
    use crate::model::var::{Objid, FAILED_MATCH, NOTHING};

    #[test]
    fn test_match_object_names_fail() {
        let mut match_data = MatchData {
            exact: NOTHING,
            partial: FAILED_MATCH,
        };

        let names = vec!["apple", "banana", "cherry"];
        let match_name = "durian";

        let result = do_match_object_names(
            Objid(2),
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
            Objid(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Objid(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: Objid(2),
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
            Objid(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Objid(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: NOTHING,
                partial: Objid(2),
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
            Objid(2),
            &mut match_data,
            names.into_iter().map(String::from).collect(),
            match_name,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Objid(2));
        assert_eq!(
            match_data,
            MatchData {
                exact: NOTHING,
                partial: Objid(2),
            }
        );
    }

    #[test]
    fn test_match_object_empty() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "");
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_match_object_object_number() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "#4");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_match_object_me() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "me");
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[test]
    fn test_match_object_here() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "here");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_room_name() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "room1");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_room_alias() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "r1");
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[test]
    fn test_match_object_player_name() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "porcupine");
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[test]
    fn test_match_object_thing_name() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "thing1");
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[test]
    fn test_match_object_thing_alias() {
        let mut env = setup_mock_environment();
        let result = world_environment_match_object(&mut env, MOCK_PLAYER, "t2");
        assert_eq!(result.unwrap(), Some(MOCK_THING2));
    }

    #[test]
    fn test_match_object_invalid_player() {
        let mut env = setup_mock_environment();
        let _player = Objid(0);
        let result = world_environment_match_object(&mut env, NOTHING, "thing1");
        assert!(result.is_err());
    }
}
