use crate::db::matching::MatchEnvironment;
use anyhow::Error;

use crate::db::CommitResult;
use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;

use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::model::var::{Objid, Var};
use crate::model::verbs::VerbInfo;
use crate::util::bitenum::BitEnum;

use crate::vm::opcode::Binary;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Verb not found: #{0:?}:{1}")]
    VerbNotFound(Objid, String),
    #[error("Invalid verb, decode error: #{0:?}:{1}")]
    VerbDecodeError(Objid, String),
    #[error("Verb permission denied: #{0:?}:{1}")]
    VerbPermissionDenied(Objid, String),
    #[error("Property not found: #{0:?}:{1}")]
    PropertyNotFound(Objid, String),
    #[error("Property permission denied: #{0:?}:{1}")]
    PropertyPermissionDenied(Objid, String),
    #[error("Object not found: #{0:?}")]
    ObjectNotFoundError(Objid),
    #[error("Failed object match: {0}")]
    FailedMatch(String),
    #[error("Ambiguous object match: {0}")]
    AmbiguousMatch(String),
}

pub trait WorldState {
    // Get the location of the given object.
    fn location_of(&mut self, obj: Objid) -> Result<Objid, anyhow::Error>;

    // Get the contents of a given object.
    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, anyhow::Error>;

    // Retrieve a verb/method from the given object.
    fn retrieve_verb(
        &mut self,
        obj: Objid,
        vname: &str,
    ) -> Result<(Binary, VerbInfo), anyhow::Error>;

    // Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &mut self,
        obj: Objid,
        pname: &str,
        player_flags: BitEnum<ObjFlag>,
    ) -> Result<Var, anyhow::Error>;

    // Update a property on the given object.
    fn update_property(
        &mut self,
        obj: Objid,
        pname: &str,
        player_flags: BitEnum<ObjFlag>,
        value: &Var,
    ) -> Result<(), anyhow::Error>;

    // Add a property for the given object.
    fn add_property(
        &mut self,
        obj: Objid,
        pname: &str,
        owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), anyhow::Error>;

    // Get the object that is the parent of the given object.
    fn parent_of(&mut self, obj: Objid) -> Result<Objid, anyhow::Error>;

    // Check the validity of an object.
    fn valid(&mut self, obj: Objid) -> Result<bool, anyhow::Error>;

    // Get the name & aliases of an object.
    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), anyhow::Error>;

    // Commit all modifications made to the state of this world since the start of its transaction.
    // Consumes self.
    fn commit(self) -> Result<CommitResult, anyhow::Error>;

    // Rollback all modifications made to the state of this world since the start of its transaction.
    // Consumes self.
    fn rollback(self) -> Result<(), anyhow::Error>;
}

pub trait WorldStateSource {
    fn new_transaction(&mut self) -> Result<Arc<Mutex<dyn WorldState>>, Error>;
}

impl MatchEnvironment for dyn WorldState {
    fn is_valid(&mut self, oid: Objid) -> Result<bool, Error> {
        self.valid(oid)
    }

    fn get_names(&mut self, oid: Objid) -> Result<Vec<String>, Error> {
        let mut names = self.names_of(oid)?;
        let mut object_names = vec![names.0];
        object_names.append(&mut names.1);
        Ok(object_names)
    }

    fn get_surroundings(&mut self, player: Objid) -> Result<Vec<Objid>, Error> {
        let location = self.location_of(player)?;
        let mut surroundings = self.contents_of(location)?;
        surroundings.push(location);
        surroundings.push(player);

        Ok(surroundings)
    }

    fn location_of(&mut self, player: Objid) -> Result<Objid, Error> {
        self.location_of(player)
    }
}
