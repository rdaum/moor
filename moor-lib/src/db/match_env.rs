use anyhow::anyhow;
use async_trait::async_trait;

use moor_value::model::world_state::WorldState;
use moor_value::var::objid::{ObjSet, Objid};

use crate::db::matching::MatchEnvironment;

pub struct DBMatchEnvironment<'a> {
    pub(crate) ws: &'a mut dyn WorldState,
    pub(crate) perms: Objid,
}

#[async_trait]
impl<'a> MatchEnvironment for DBMatchEnvironment<'a> {
    async fn obj_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error> {
        self.ws.valid(oid).await.map_err(|e| anyhow!(e))
    }

    async fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error> {
        let mut names = self.ws.names_of(self.perms, oid).await?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    async fn get_surroundings(&mut self, player: Objid) -> Result<ObjSet, anyhow::Error> {
        let location = self.ws.location_of(self.perms, player).await?;
        let mut surroundings = self.ws.contents_of(self.perms, location).await?;
        surroundings.insert(location);
        surroundings.insert(player);

        Ok(surroundings)
    }

    async fn location_of(&mut self, player: Objid) -> Result<Objid, anyhow::Error> {
        Ok(self.ws.location_of(self.perms, player).await?)
    }
}
