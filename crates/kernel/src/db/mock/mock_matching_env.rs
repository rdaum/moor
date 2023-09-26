use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use moor_values::model::objset::ObjSet;
use moor_values::model::WorldStateError;
use moor_values::NOTHING;

use moor_values::var::objid::Objid;

use crate::db::matching::MatchEnvironment;

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

#[async_trait]
impl MatchEnvironment for MockMatchEnvironment {
    async fn obj_valid(&mut self, oid: Objid) -> Result<bool, WorldStateError> {
        Ok(self.objects.contains_key(&oid))
    }

    async fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, WorldStateError> {
        Ok(self
            .objects
            .get(&oid)
            .map_or_else(Vec::new, |o| o.names.clone()))
    }

    async fn get_surroundings(&mut self, player: Objid) -> Result<ObjSet, WorldStateError> {
        let mut result = Vec::new();
        if let Some(player_obj) = self.objects.get(&player) {
            result.push(MOCK_PLAYER);
            result.push(player_obj.location);
            result.extend(player_obj.contents.iter().cloned());

            if let Some(location_obj) = self.objects.get(&player_obj.location) {
                result.extend(location_obj.contents.iter().cloned());
            }
        }
        Ok(ObjSet::from(&result))
    }

    async fn location_of(&mut self, oid: Objid) -> Result<Objid, WorldStateError> {
        self.objects
            .get(&oid)
            .map(|o| o.location)
            .ok_or_else(|| WorldStateError::ObjectNotFound(oid))
    }
}

fn create_mock_object(
    env: &mut MockMatchEnvironment,
    oid: Objid,
    location: Objid,
    contents: ObjSet,
    names: Vec<String>,
) {
    env.objects.insert(
        oid,
        MockObject {
            location,
            contents: contents.iter().collect(),
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
        ObjSet::new(),
        vec!["porcupine".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM1,
        NOTHING,
        ObjSet::from(&[MOCK_THING1, MOCK_THING2]),
        vec!["room1".to_string(), "r1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM2,
        NOTHING,
        ObjSet::from(&[MOCK_THING3]),
        vec!["room2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING1,
        MOCK_ROOM1,
        ObjSet::new(),
        vec!["thing1".to_string(), "t1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING2,
        MOCK_ROOM1,
        ObjSet::new(),
        vec!["thing2".to_string(), "t2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING3,
        MOCK_ROOM2,
        ObjSet::new(),
        vec!["thing3".to_string(), "t3".to_string()],
    );

    env
}
