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

use std::collections::{HashMap, HashSet};

use crate::model::{ObjSet, ValSet};
use crate::model::{ObjectRef, WorldStateError};
use moor_var::NOTHING;
use moor_var::Obj;

use crate::matching::match_env::MatchEnvironment;

pub const MOCK_PLAYER: Obj = Obj::mk_id(3);
pub const MOCK_ROOM1: Obj = Obj::mk_id(1);
pub const MOCK_ROOM2: Obj = Obj::mk_id(2);
pub const MOCK_THING1: Obj = Obj::mk_id(4);
pub const MOCK_THING2: Obj = Obj::mk_id(5);
pub const MOCK_THING3: Obj = Obj::mk_id(6);

pub struct MockObject {
    pub location: Obj,
    pub contents: HashSet<Obj>,
    pub names: Vec<String>,
}

#[derive(Default)]
pub struct MockMatchEnv {
    objects: HashMap<Obj, MockObject>,
}

impl MockMatchEnv {
    pub fn new(objects: HashMap<Obj, MockObject>) -> Self {
        MockMatchEnv { objects }
    }
}

impl MatchEnvironment for MockMatchEnv {
    fn obj_valid(&self, oid: &Obj) -> Result<bool, WorldStateError> {
        Ok(self.objects.contains_key(oid))
    }

    fn get_names(&self, oid: &Obj) -> Result<Vec<String>, WorldStateError> {
        Ok(self
            .objects
            .get(oid)
            .map_or_else(Vec::new, |o| o.names.clone()))
    }

    fn get_surroundings(&self, player: &Obj) -> Result<ObjSet, WorldStateError> {
        let mut result = Vec::new();
        if let Some(player_obj) = self.objects.get(player) {
            result.push(MOCK_PLAYER);
            result.push(player_obj.location.clone());
            result.extend(player_obj.contents.iter().cloned());

            if let Some(location_obj) = self.objects.get(&player_obj.location) {
                result.extend(location_obj.contents.iter().cloned());
            }
        }
        Ok(ObjSet::from_items(&result))
    }

    fn location_of(&self, oid: &Obj) -> Result<Obj, WorldStateError> {
        self.objects
            .get(oid)
            .map(|o| o.location.clone())
            .ok_or(WorldStateError::ObjectNotFound(ObjectRef::Id(oid.clone())))
    }
}

fn create_mock_object(
    env: &mut MockMatchEnv,
    oid: Obj,
    location: Obj,
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

pub fn setup_mock_environment() -> MockMatchEnv {
    let mut env = MockMatchEnv::default();

    create_mock_object(
        &mut env,
        MOCK_PLAYER,
        MOCK_ROOM1,
        ObjSet::empty(),
        vec!["porcupine".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM1,
        NOTHING,
        ObjSet::from_items(&[MOCK_THING1, MOCK_THING2]),
        vec!["room1".to_string(), "r1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_ROOM2,
        NOTHING,
        ObjSet::from_items(&[MOCK_THING3]),
        vec!["room2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING1,
        MOCK_ROOM1,
        ObjSet::empty(),
        vec!["thing1".to_string(), "t1".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING2,
        MOCK_ROOM1,
        ObjSet::empty(),
        vec!["thing2".to_string(), "t2".to_string()],
    );
    create_mock_object(
        &mut env,
        MOCK_THING3,
        MOCK_ROOM2,
        ObjSet::empty(),
        vec!["thing3".to_string(), "t3".to_string()],
    );

    env
}
