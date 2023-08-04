use anyhow::anyhow;
use async_trait::async_trait;

use moor_value::var::objid::FAILED_MATCH;
use moor_value::var::objid::{Objid, AMBIGUOUS, NOTHING};

use crate::model::ObjectError;
use crate::tasks::command_parse::ParseMatcher;

// This is the interface that the matching code needs to be able to call into the world state.
// Separated out so can be more easily mocked.
#[async_trait]
pub trait MatchEnvironment {
    // Test whether a given object is valid in this environment.
    async fn obj_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error>;

    // Return all match names & aliases for an object.
    async fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error>;

    // Returns location, contents, and player, all the things we'd search for matches on.
    async fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, anyhow::Error>;

    // Return the location of a given object.
    async fn location_of(&mut self, player: Objid) -> Result<Objid, anyhow::Error>;
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

pub async fn match_contents<M: MatchEnvironment + Send + Sync>(
    env: &mut M,
    player: Objid,
    object_name: &str,
) -> Result<Option<Objid>, anyhow::Error> {
    let mut match_data = MatchData {
        exact: NOTHING,
        partial: FAILED_MATCH,
    };

    let search = env.get_surroundings(player).await?; // location, contents, player
    for oid in search {
        if !env.obj_valid(oid).await? {
            continue;
        }

        let object_names = env.get_names(oid).await?;
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

pub struct MatchEnvironmentParseMatcher<M: MatchEnvironment + Send + Sync> {
    pub env: M,
    pub player: Objid,
}

#[async_trait]
impl<M: MatchEnvironment + Send + Sync> ParseMatcher for MatchEnvironmentParseMatcher<M> {
    async fn match_object(&mut self, object_name: &str) -> Result<Option<Objid>, anyhow::Error> {
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
        if !self.env.obj_valid(self.player).await? {
            return Err(anyhow!(ObjectError::FailedMatch(
                "Invalid current player when performing object match".to_string()
            )));
        }

        // Check 'me' and 'here' first.
        if object_name == "me" {
            return Ok(Some(self.player));
        }

        if object_name == "here" {
            return Ok(Some(self.env.location_of(self.player).await?));
        }

        match_contents(&mut self.env, self.player, object_name).await
    }
}

#[cfg(test)]
mod tests {
    use moor_value::var::objid::FAILED_MATCH;
    use moor_value::var::objid::{Objid, NOTHING};

    use crate::db::matching::{do_match_object_names, MatchData, MatchEnvironmentParseMatcher};
    use crate::db::mock_matching_env::{
        setup_mock_environment, MOCK_PLAYER, MOCK_ROOM1, MOCK_THING1, MOCK_THING2,
    };
    use crate::tasks::command_parse::ParseMatcher;

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

    #[tokio::test]
    async fn test_match_object_empty() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("").await;
        assert_eq!(result.unwrap(), None);
    }

    #[tokio::test]
    async fn test_match_object_object_number() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("#4").await;
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[tokio::test]
    async fn test_match_object_me() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("me").await;
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[tokio::test]
    async fn test_match_object_here() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("here").await;
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[tokio::test]
    async fn test_match_object_room_name() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("room1").await;
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[tokio::test]
    async fn test_match_object_room_alias() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("r1").await;
        assert_eq!(result.unwrap(), Some(MOCK_ROOM1));
    }

    #[tokio::test]
    async fn test_match_object_player_name() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("porcupine").await;
        assert_eq!(result.unwrap(), Some(MOCK_PLAYER));
    }

    #[tokio::test]
    async fn test_match_object_thing_name() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("thing1").await;
        assert_eq!(result.unwrap(), Some(MOCK_THING1));
    }

    #[tokio::test]
    async fn test_match_object_thing_alias() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: MOCK_PLAYER,
        };
        let result = menv.match_object("t2").await;
        assert_eq!(result.unwrap(), Some(MOCK_THING2));
    }

    #[tokio::test]
    async fn test_match_object_invalid_player() {
        let env = setup_mock_environment();
        let mut menv = MatchEnvironmentParseMatcher {
            env,
            player: NOTHING,
        };
        let result = menv.match_object("thing1").await;
        assert!(result.is_err());
    }
}
