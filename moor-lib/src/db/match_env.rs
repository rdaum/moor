use anyhow::anyhow;

use crate::db::matching::MatchEnvironment;
use crate::db::state::WorldState;
use crate::var::Objid;

pub struct DBMatchEnvironment<'a> {
    pub(crate) ws: &'a mut dyn WorldState,
}

impl<'a> MatchEnvironment for DBMatchEnvironment<'a> {
    fn obj_valid(&mut self, oid: Objid) -> Result<bool, anyhow::Error> {
        self.ws.valid(oid).map_err(|e| anyhow!(e))
    }

    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, anyhow::Error> {
        let mut names = self.ws.names_of(oid)?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, anyhow::Error> {
        let location = self.ws.location_of(player)?;
        let mut surroundings = self.ws.contents_of(location)?;
        surroundings.push(location);
        surroundings.push(player);

        Ok(surroundings)
    }

    fn location_of(&mut self, player: Objid) -> Result<Objid, anyhow::Error> {
        Ok(self.ws.location_of(player)?)
    }
}
