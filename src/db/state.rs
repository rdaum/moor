use std::marker::PhantomData;
use crate::db::matching::MatchEnvironment;
use anyhow::{anyhow, Error};
use bincode::config;
use bincode::error::DecodeError;
use enumset::EnumSet;
use rusqlite::Transaction;
use thiserror::Error;

use crate::model::objects::{ObjAttr, ObjFlag};
use crate::model::props::{PropAttr, PropFlag};
use crate::model::var::{Objid, Var, AMBIGUOUS, FAILED_MATCH, NOTHING};
use crate::model::verbs::{VerbAttr, VerbInfo};
use crate::model::ObjDB;
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
    fn retrieve_verb(&self, obj: Objid, vname: &str) -> Result<(Binary, VerbInfo), anyhow::Error>;

    // Retrieve a property from the given object, walking transitively up its inheritance chain.
    fn retrieve_property(
        &self,
        obj: Objid,
        pname: &str,
        player_flags: EnumSet<ObjFlag>,
    ) -> Result<Var, anyhow::Error>;

    // Update a property on the given object.
    fn update_property(
        &mut self,
        obj: Objid,
        pname: &str,
        player_flags: EnumSet<ObjFlag>,
        value: &Var,
    ) -> Result<(), anyhow::Error>;

    // Get the object that is the parent of the given object.
    fn parent_of(&mut self, obj: Objid) -> Result<Objid, anyhow::Error>;

    // Check the validity of an object.
    fn valid(&mut self, obj: Objid) -> Result<bool, anyhow::Error>;

    // Get the name & aliases of an object.
    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), anyhow::Error>;

    // Commit all modifications made to the state of this world since the start of its transaction.
    // The world state becomes inoperable after this
    fn commit(&mut self) -> Result<(), anyhow::Error>;

    // Rollback all modifications made to the state of this world since the start of its transaction.
    // The world state becomes inoperable after this
    fn rollback(&mut self) -> Result<(), anyhow::Error>;
}

pub trait WorldStateSource {
    fn new_transaction(&mut self) -> Result<Box<dyn WorldState + '_>, Error>;
}


pub struct ObjDBState<T>
    where T: ObjDB
{
    pub db: T,
}

impl<T: ObjDB> ObjDBState<T>

{
    pub fn new(db: T) -> Self
    {
        Self {
            db
        }
    }
}

impl <T: ObjDB> WorldState for ObjDBState<T> {
    fn location_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        self.db
            .object_get_attrs(obj, EnumSet::from(ObjAttr::Location))
            .map(|attrs| attrs.location.unwrap())
    }

    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, Error> {
        self.db.object_contents(obj)
    }

    fn retrieve_verb(&self, obj: Objid, vname: &str) -> Result<(Binary, VerbInfo), Error> {
        let h = self.db.find_callable_verb(
            obj,
            vname,
            VerbAttr::Program | VerbAttr::Flags | VerbAttr::Owner | VerbAttr::Definer,
        )?;
        let Some(vi) = h else {
            return Err(anyhow!(StateError::VerbNotFound(obj, vname.to_string())));
        };

        let program = vi.clone().attrs.program.unwrap();
        let slc = &program.0[..];
        let result: Result<(Binary, usize), DecodeError> =
            bincode::serde::decode_from_slice(slc, config::standard());
        let Ok((binary, _size)) = result else {
            return Err(anyhow!(StateError::VerbDecodeError(obj, vname.to_string())));
        };

        Ok((binary, vi))
    }

    fn retrieve_property(
        &self,
        obj: Objid,
        property_name: &str,
        player_flags: EnumSet<ObjFlag>,
    ) -> Result<Var, Error> {
        // TODO builtin properties!

        let find = self
            .db
            .find_property(
                obj,
                property_name,
                PropAttr::Owner | PropAttr::Flags | PropAttr::Location | PropAttr::Value,
            )
            .expect("db fail");
        match find {
            None => Err(anyhow!(StateError::PropertyNotFound(
                obj,
                property_name.to_string()
            ))),
            Some(p) => {
                if !self.db.property_allows(
                    PropFlag::Read.into(),
                    obj,
                    player_flags,
                    p.attrs.flags.unwrap(),
                    p.attrs.owner.unwrap(),
                ) {
                    Err(anyhow!(StateError::PropertyPermissionDenied(
                        obj,
                        property_name.to_string()
                    )))
                } else {
                    match p.attrs.value {
                        None => Err(anyhow!(StateError::PropertyNotFound(
                            obj,
                            property_name.to_string()
                        ))),
                        Some(p) => Ok(p),
                    }
                }
            }
        }
    }

    fn update_property(
        &mut self,
        obj: Objid,
        property_name: &str,
        player_flags: EnumSet<ObjFlag>,
        value: &Var,
    ) -> Result<(), Error> {
        let h = self
            .db
            .find_property(obj, property_name, PropAttr::Owner | PropAttr::Flags)
            .expect("Unable to perform property lookup");

        // TODO handle built-in properties
        let Some(p) = h else {
            return Err(anyhow!(StateError::PropertyNotFound(obj, property_name.to_string())));
        };

        if self.db.property_allows(
            PropFlag::Write.into(),
            obj,
            player_flags,
            p.attrs.flags.unwrap(),
            p.attrs.owner.unwrap(),
        ) {
            return Err(anyhow!(StateError::PropertyPermissionDenied(
                obj,
                property_name.to_string()
            )));
        }

        // Failure on this is a panic.
        self.db
            .set_property(
                p.pid,
                obj,
                value.clone(),
                p.attrs.owner.unwrap(),
                p.attrs.flags.unwrap(),
            )
            .expect("could not set property");
        Ok(())
    }

    fn parent_of(&mut self, obj: Objid) -> Result<Objid, Error> {
        let attrs = self
            .db
            .object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner)?;
        // TODO: this masks other (internal?) errors..
        attrs
            .parent
            .ok_or(anyhow!(StateError::ObjectNotFoundError(obj)))
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, Error> {
        // TODO: this masks other (internal?) errors..
        Ok(self
            .db
            .object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner)
            .is_ok())
    }

    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), Error> {
        // TODO implement support for aliases.
        let name = self
            .db
            .object_get_attrs(obj, EnumSet::from(ObjAttr::Name))?
            .name
            .unwrap();
        return Ok((name, vec![]));
    }

    fn commit(&mut self)  -> Result<(), anyhow::Error> {
        self.db.commit()
    }

    fn rollback(&mut self)  -> Result<(), anyhow::Error> {
        self.db.rollback()
    }
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
