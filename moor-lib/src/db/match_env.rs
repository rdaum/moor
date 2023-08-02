use anyhow::anyhow;

use crate::db::matching::MatchEnvironment;
use crate::model::permissions::PermissionsContext;
use crate::model::world_state::WorldState;
use crate::values::objid::Objid;

pub struct DBMatchEnvironment<'a> {
    pub(crate) ws: &'a mut dyn WorldState,
    pub(crate) perms: PermissionsContext,
}

impl<'a> MatchEnvironment for DBMatchEnvironment<'a> {
    fn obj_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error> {
        self.ws
            .valid(self.perms.clone(), oid)
            .map_err(|e| anyhow!(e))
    }

    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error> {
        let mut names = self.ws.names_of(self.perms.clone(), oid)?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let location = self.ws.location_of(self.perms.clone(), player)?;
        let mut surroundings = self.ws.contents_of(self.perms.clone(), location)?;
        surroundings.push(location);
        surroundings.push(player);

        Ok(surroundings)
    }

    fn location_of(&mut self, player: Objid) -> Result<Objid, anyhow::Error> {
        Ok(self.ws.location_of(self.perms.clone(), player)?)
    }
}
