use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};

use crate::db::matching::MatchEnvironment;
use crate::model::var::{Objid, NOTHING};

pub const MOCK_PLAYER: Objid = Objid(3);
pub const MOCK_ROOM1: Objid = Objid(1);
pub const MOCK_ROOM2: Objid = Objid(2);
pub const MOCK_THING1: Objid = Objid(4);
pub const MOCK_THING2: Objid = Objid(5);
pub const MOCK_THING3: Objid = Objid(6);

pub struct MockObject {
    pub location: Objid,
    pub contents: HashSet<Objid>,
    pub names: Vec<String>,
}

#[derive(Default)]
pub struct MockMatchEnvironment {
    objects: HashMap<Objid, MockObject>,
}

impl MockMatchEnvironment {
    pub fn new(objects: HashMap<Objid, MockObject>) -> Self {
        MockMatchEnvironment { objects }
    }
}

impl MatchEnvironment for MockMatchEnvironment {
    fn is_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error> {
        Ok(self.objects.contains_key(&oid))
    }

    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error> {
        Ok(self
            .objects
            .get(&oid)
            .map_or_else(Vec::new, |o| o.names.clone()))
    }

    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let mut result = Vec::new();
        if let Some(player_obj) = self.objects.get(&player) {
            result.push(MOCK_PLAYER);
            result.push(player_obj.location);
            result.extend(player_obj.contents.iter().cloned());

            if let Some(location_obj) = self.objects.get(&player_obj.location) {
                result.extend(location_obj.contents.iter().cloned());
            }
        }
        Ok(result)
    }

    fn location_of(&mut self, oid: Objid) -> Result<Objid, anyhow::Error> {
        self.objects
            .get(&oid)
            .map(|o| o.location)
            .ok_or_else(|| anyhow!("Object not found: {:?}", oid))
    }
}

fn create_mock_object(
    env: &mut MockMatchEnvironment,
    oid: Objid,
    location: Objid,
    contents: Vec<Objid>,
    names: Vec<String>,
) {
    env.objects.insert(
        oid,
        MockObject {
            location,
            contents: contents.into_iter().collect(),
            names,
        },
    );
}

pub fn setup_mock_environment() -> MockMatchEnvironment {
    let mut env = MockMatchEnvironment::default();

    create_mock_object(
        &mut env,
        MOCK_PLAYER,
        MOCK_ROOM1,
        vec![],
        vec!["porcupine".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM1,
        NOTHING,
        vec![MOCK_THING1, MOCK_THING2],
        vec!["room1".to_string(), "r1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM2,
        NOTHING,
        vec![MOCK_THING3],
        vec!["room2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING1,
        MOCK_ROOM1,
        vec![],
        vec!["thing1".to_string(), "t1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING2,
        MOCK_ROOM1,
        vec![],
        vec!["thing2".to_string(), "t2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING3,
        MOCK_ROOM2,
        vec![],
        vec!["thing3".to_string(), "t3".to_string()],
    );

    env
}
