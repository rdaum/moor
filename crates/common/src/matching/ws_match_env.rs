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

use crate::model::ObjSet;
use crate::model::WorldState;
use crate::model::WorldStateError;
use crate::Obj;

use crate::matching::match_env::MatchEnvironment;

/// A "match environment" which matches out of the current DB world state.
pub struct WsMatchEnv<'a> {
    pub(crate) ws: &'a dyn WorldState,
    pub(crate) perms: Obj,
}

impl<'a> WsMatchEnv<'a> {
    pub fn new(ws: &'a dyn WorldState, perms: Obj) -> Self {
        Self { ws, perms }
    }
}
impl MatchEnvironment for WsMatchEnv<'_> {
    fn obj_valid(&self, oid: &Obj) -> Result<bool, WorldStateError> {
        self.ws.valid(oid)
    }

    fn get_names(&self, oid: &Obj) -> Result<Vec<String>, WorldStateError> {
        let mut names = self.ws.names_of(&self.perms, oid)?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    fn get_surroundings(&self, player: &Obj) -> Result<ObjSet, WorldStateError> {
        let location = self.ws.location_of(&self.perms, player)?;
        let surroundings = self
            .ws
            .contents_of(&self.perms, &location)?
            .with_appended(&[location, player.clone()]);

        Ok(surroundings)
    }

    fn location_of(&self, player: &Obj) -> Result<Obj, WorldStateError> {
        self.ws.location_of(&self.perms, player)
    }
}
