use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag};
use crate::model::props::{PropAttr, PropFlag};
use crate::model::var::Error::E_VERBNF;
use crate::model::var::{Objid, Var};
use crate::model::verbs::{VerbAttr, VerbAttrs, VerbInfo};
use crate::model::ObjDB;
use crate::vm::opcode::Binary;
use anyhow::{anyhow, Error};
use bincode::config;
use bincode::error::DecodeError;
use enumset::EnumSet;
use thiserror::Error;

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
}

pub trait PersistentState {
    fn retrieve_verb(
        &self,
        obj: Objid,
        vname: &str,
        do_pass: bool,
        this: Objid,
        player: Objid,
        caller: Objid,
        args: &Vec<Var>,
    ) -> Result<(Binary, VerbInfo), anyhow::Error>;
    fn retrieve_property(
        &self,
        obj: Objid,
        pname: &str,
        player_flags: EnumSet<ObjFlag>,
    ) -> Result<Var, anyhow::Error>;
    fn update_property(
        &mut self,
        obj: Objid,
        pname: &str,
        player_flags: EnumSet<ObjFlag>,
        value: &Var,
    ) -> Result<(), anyhow::Error>;
    fn parent_of(
        &mut self,
        obj: Objid) -> Result<Objid, anyhow::Error>;
    fn valid(
        &mut self,
        obj: Objid) -> Result<bool, anyhow::Error>;
}

pub struct ObjDBState<'a> {
    pub db: &'a mut dyn ObjDB,
}

impl<'a> PersistentState for ObjDBState<'a> {
    fn retrieve_verb(
        &self,
        obj: Objid,
        vname: &str,
        do_pass: bool,
        _this: Objid,
        _player: Objid,
        _caller: Objid,
        _args: &Vec<Var>,
    ) -> Result<(Binary, VerbInfo), Error> {
        // TODO do_pass get parent and delegate there instead.
        // Requires adding db.object_parent.
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
            None => Err(anyhow!(StateError::PropertyNotFound(obj, property_name.to_string()))),
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
                        None => Err(anyhow!(StateError::PropertyNotFound(obj, property_name.to_string()))),
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
        let attrs = self.db.object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner)?;
        // TODO: this masks other (internal?) errors..
        attrs.parent.ok_or(anyhow!(StateError::ObjectNotFoundError(obj)))
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, Error> {
        // TODO: this masks other (internal?) errors..
        Ok(self.db.object_get_attrs(obj, ObjAttr::Parent | ObjAttr::Owner).is_ok())
    }
}
