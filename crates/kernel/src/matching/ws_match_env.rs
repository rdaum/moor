use async_trait::async_trait;
use moor_values::model::objset::ObjSet;

use moor_values::model::world_state::WorldState;
use moor_values::model::WorldStateError;
use moor_values::var::objid::Objid;

use crate::matching::match_env::MatchEnvironment;

/// A "match environment" which matches out of the current DB world state.
pub struct WsMatchEnv<'a> {
    pub(crate) ws: &'a mut dyn WorldState,
    pub(crate) perms: Objid,
}

#[async_trait]
impl<'a> MatchEnvironment for WsMatchEnv<'a> {
    async fn obj_valid(&mut self, oid: Objid) -> Result<bool, WorldStateError> {
        self.ws.valid(oid).await
    }

    async fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, WorldStateError> {
        let mut names = self.ws.names_of(self.perms, oid).await?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    async fn get_surroundings(&mut self, player: Objid) -> Result<ObjSet, WorldStateError> {
        let location = self.ws.location_of(self.perms, player).await?;
        let surroundings = self
            .ws
            .contents_of(self.perms, location)
            .await?
            .with_appended(&[location, player]);

        Ok(surroundings)
    }

    async fn location_of(&mut self, player: Objid) -> Result<Objid, WorldStateError> {
        self.ws.location_of(self.perms, player).await
    }
}
