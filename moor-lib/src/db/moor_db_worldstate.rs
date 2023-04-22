use std::sync::atomic::AtomicPtr;

use anyhow::anyhow;

use tuplebox::relations;
use tuplebox::tx::Tx;

use crate::db::CommitResult;
use crate::db::moor_db::MoorDB;
use crate::db::state::{WorldState, WorldStateSource};
use crate::model::ObjectError;
use crate::model::ObjectError::{
    ObjectNotFound, PropertyNotFound, PropertyPermissionDenied, VerbDecodeError, VerbNotFound,
};
use crate::model::objects::{ObjAttr, ObjAttrs, Objects, ObjFlag};
use crate::model::permissions::Permissions;
use crate::model::props::{
    Pid, PropAttr, PropAttrs, Propdef, PropDefs, Properties, PropertyInfo, PropFlag,
};
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::var::{NOTHING, Objid, Var, v_list, v_bool};
use crate::model::verbs::{VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::tasks::parse_cmd::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::Binary;

pub struct MoorDbWorldStateSource {
    db: MoorDB,
}
unsafe impl Send for MoorDbWorldStateSource {}
unsafe impl Sync for MoorDbWorldStateSource {}

impl MoorDbWorldStateSource {
    pub fn new(db: MoorDB) -> Self {
        MoorDbWorldStateSource { db }
    }
}

impl WorldStateSource for MoorDbWorldStateSource {
    fn new_world_state(&mut self) -> Result<Box<dyn WorldState>, anyhow::Error> {
        let tx = self.db.do_begin_tx()?;
        let odb = MoorDbTx::boxed(AtomicPtr::new(&mut self.db), tx);
        Ok(odb)
    }
}

pub struct MoorDbTx {
    db: AtomicPtr<MoorDB>,
    tx: Tx,
}

impl MoorDbTx {
    pub fn boxed(db: AtomicPtr<MoorDB>, tx: Tx) -> Box<dyn WorldState> {
        Box::new(MoorDbTx { db, tx })
    }

    pub fn get_db<'a>(&mut self) -> &'a mut MoorDB {
        unsafe { return self.db.get_mut().as_mut().unwrap() }
    }
}

// The DB itself attempts to be thread-safe, there's no need to hold it in an Arc/Mutex.
// We can the tx handle here send and sync-able, and it can be passed around and traded.
unsafe impl Send for MoorDbTx {}
unsafe impl Sync for MoorDbTx {}

impl Objects for MoorDbTx {
    fn create_object(
        &mut self,
        oid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, ObjectError> {
        let db = self.get_db();
        db.create_object(&mut self.tx, oid, attrs)
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), ObjectError> {
        self.get_db().destroy_object(&mut self.tx, oid)
    }

    fn object_valid(&mut self, oid: Objid) -> Result<bool, ObjectError> {
        self.get_db().object_valid(&mut self.tx, oid)
    }

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: BitEnum<ObjAttr>,
    ) -> Result<ObjAttrs, ObjectError> {
        self.get_db()
            .object_get_attrs(&mut self.tx, oid, attributes)
    }

    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), ObjectError> {
        self.get_db()
            .object_set_attrs(&mut self.tx, oid, attributes)
    }

    fn object_children(&mut self, oid: Objid) -> Result<Vec<Objid>, ObjectError> {
        self.get_db().object_children(&mut self.tx, oid)
    }

    fn object_contents(&mut self, oid: Objid) -> Result<Vec<Objid>, ObjectError> {
        self.get_db().object_contents(&mut self.tx, oid)
    }
}

impl Properties for MoorDbTx {
    fn find_property(
        &mut self,
        oid: Objid,
        name: &str,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropertyInfo>, ObjectError> {
        self.get_db().find_property(&mut self.tx, oid, name, attrs)
    }

    fn get_property(
        &mut self,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, ObjectError> {
        self.get_db().get_property(&mut self.tx, oid, handle, attrs)
    }

    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), ObjectError> {
        self.get_db()
            .set_property(&mut self.tx, handle, location, value, owner, flags)
    }
}

impl Verbs for MoorDbTx {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Binary,
    ) -> Result<VerbInfo, ObjectError> {
        self.get_db()
            .add_verb(&mut self.tx, oid, names, owner, flags, arg_spec, program)
    }

    fn get_verbs(
        &mut self,
        oid: Objid,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Vec<VerbInfo>, ObjectError> {
        self.get_db().get_verbs(&mut self.tx, oid, attrs)
    }

    fn get_verb(&mut self, vid: Vid, attrs: BitEnum<VerbAttr>) -> Result<VerbInfo, ObjectError> {
        self.get_db().get_verb(&mut self.tx, vid, attrs)
    }

    fn update_verb(&mut self, vid: Vid, attrs: VerbAttrs) -> Result<(), ObjectError> {
        self.get_db().update_verb(&mut self.tx, vid, attrs)
    }

    fn find_command_verb(
        &mut self,
        obj: Objid,
        verb: &str,
        dobj: ArgSpec,
        prep: PrepSpec,
        iobj: ArgSpec,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        self.get_db()
            .find_command_verb(&mut self.tx, obj, verb, dobj, prep, iobj)
    }

    fn find_callable_verb(
        &mut self,
        oid: Objid,
        verb: &str,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        self.get_db()
            .find_callable_verb(&mut self.tx, oid, verb, attrs)
    }

    fn find_indexed_verb(
        &mut self,
        oid: Objid,
        index: usize,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        self.get_db()
            .find_indexed_verb(&mut self.tx, oid, index, attrs)
    }
}

impl Permissions for MoorDbTx {
    fn property_allows(
        &mut self,
        check_flags: BitEnum<PropFlag>,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        prop_flags: BitEnum<PropFlag>,
        prop_owner: Objid,
    ) -> bool {
        self.get_db().property_allows(
            &mut self.tx,
            check_flags,
            player,
            player_flags,
            prop_flags,
            prop_owner,
        )
    }
}

impl PropDefs for MoorDbTx {
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, ObjectError> {
        self.get_db().get_propdef(&mut self.tx, definer, pname)
    }

    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, ObjectError> {
        self.get_db()
            .add_propdef(&mut self.tx, definer, name, owner, flags, initial_value)
    }

    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), ObjectError> {
        self.get_db()
            .rename_propdef(&mut self.tx, definer, old, new)
    }

    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), ObjectError> {
        self.get_db().delete_propdef(&mut self.tx, definer, pname)
    }

    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, ObjectError> {
        self.get_db().count_propdefs(&mut self.tx, definer)
    }

    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, ObjectError> {
        self.get_db().get_propdefs(&mut self.tx, definer)
    }
}

fn builtin_propname_to_objattr(name: &str) -> Option<ObjAttr> {
    match name {
        "name" => Some(ObjAttr::Name),
        "location" => Some(ObjAttr::Location),
        "owner" => Some(ObjAttr::Owner),
        "parent" => Some(ObjAttr::Parent),
        _ => None,
    }
}

impl WorldState for MoorDbTx {
    fn location_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        self.object_get_attrs(obj, BitEnum::new_with(ObjAttr::Location))
            .map(|attrs| attrs.location.unwrap())
    }

    fn contents_of(&mut self, obj: Objid) -> Result<Vec<Objid>, ObjectError> {
        self.object_contents(obj)
    }

    fn flags_of(&mut self, obj: Objid) -> Result<BitEnum<ObjFlag>, ObjectError> {
        let obj_attrs = self.object_get_attrs(obj, BitEnum::new_with(ObjAttr::Flags))?;
        obj_attrs.flags.ok_or(ObjectError::ObjectNotFound(obj))
    }

    fn verbs(&mut self, obj: Objid) -> Result<Vec<VerbInfo>, ObjectError> {
        self.get_verbs(obj, BitEnum::all())
    }

    fn properties(&mut self, obj: Objid) -> Result<Vec<(String, PropAttrs)>, ObjectError> {
        let propdefs = self.get_propdefs(obj)?;
        let properties = propdefs.into_iter().map(|pd| {
            (
                pd.pname.clone(),
                self.get_property(obj, pd.pid, BitEnum::all())
                    .unwrap()
                    .unwrap(),
            )
        });
        Ok(properties.collect())
    }

    fn retrieve_verb(
        &mut self,
        obj: Objid,
        vname: &str,
    ) -> Result<(Binary, VerbInfo), ObjectError> {
        let h = self.find_callable_verb(
            obj,
            vname,
            BitEnum::new_with(VerbAttr::Program)
                | VerbAttr::Flags
                | VerbAttr::Owner
                | VerbAttr::Definer,
        )?;
        let Some(vi) = h else {
            return Err(VerbNotFound(obj, vname.to_string()));
        };

        let Some(binary) = &vi.attrs.program else {
            return Err(VerbDecodeError(obj, vname.to_string()));
        };

        Ok((binary.clone(), vi))
    }

    fn retrieve_property(
        &mut self,
        obj: Objid,
        property_name: &str,
        player_flags: BitEnum<ObjFlag>,
    ) -> Result<Var, ObjectError> {
        if let Some(builtin) = builtin_propname_to_objattr(property_name) {
            let attrs = self.object_get_attrs(obj, BitEnum::new_with(builtin))?;
            return Ok(match builtin {
                ObjAttr::Name => Var::Str(attrs.name.unwrap()),
                ObjAttr::Location => Var::Obj(attrs.location.unwrap()),
                ObjAttr::Owner => Var::Obj(attrs.owner.unwrap()),
                ObjAttr::Parent => Var::Obj(attrs.parent.unwrap()),
                _ => unreachable!(),
            });
        }
        if property_name == "contents" {
            return Ok(v_list(
                self.object_contents(obj)?
                    .into_iter()
                    .map(Var::Obj)
                    .collect(),
            ));
        }
        if property_name == "wizard" {
            let is_wizard = player_flags.contains(ObjFlag::Wizard);
            return Ok(v_bool(is_wizard));
        }
        if property_name == "programmer" {
            let is_programmer = player_flags.contains(ObjFlag::Programmer);
            return Ok(v_bool(is_programmer));
        }
        if property_name == "r" {
            let readable = player_flags.contains(ObjFlag::Read);
            return Ok(v_bool(readable));
        }
        if property_name == "w" {
            let writable = player_flags.contains(ObjFlag::Write);
            return Ok(v_bool(writable));
        }
        if property_name == "f" {
            let forceable = player_flags.contains(ObjFlag::Fertile);
            return Ok(v_bool(forceable));
        }

        let find = self.find_property(
            obj,
            property_name,
            BitEnum::new_with(PropAttr::Owner)
                | PropAttr::Flags
                | PropAttr::Location
                | PropAttr::Value,
        )?;
        match find {
            None => Err(PropertyNotFound(obj, property_name.to_string())),
            Some(p) => {
                if !self.property_allows(
                    PropFlag::Read.into(),
                    obj,
                    player_flags,
                    p.attrs.flags.unwrap(),
                    p.attrs.owner.unwrap(),
                ) {
                    Err(PropertyPermissionDenied(obj, property_name.to_string()))
                } else {
                    match p.attrs.value {
                        None => Err(PropertyNotFound(obj, property_name.to_string())),
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
        player_flags: BitEnum<ObjFlag>,
        value: &Var,
    ) -> Result<(), ObjectError> {
        let h = self
            .find_property(
                obj,
                property_name,
                BitEnum::new_with(PropAttr::Owner) | PropAttr::Flags,
            )
            .expect("Unable to perform property lookup");

        let Some(p) = h else {
            return Err(PropertyNotFound(obj, property_name.to_string()));
        };

        if self.property_allows(
            PropFlag::Write.into(),
            obj,
            player_flags,
            p.attrs.flags.unwrap(),
            p.attrs.owner.unwrap(),
        ) {
            return Err(PropertyPermissionDenied(obj, property_name.to_string()));
        }

        // Failure on this is a panic.
        self.set_property(
            p.pid,
            obj,
            value.clone(),
            p.attrs.owner.unwrap(),
            p.attrs.flags.unwrap(),
        )
        .expect("could not set property");
        Ok(())
    }

    fn add_property(
        &mut self,
        obj: Objid,
        pname: &str,
        _owner: Objid,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), ObjectError> {
        self.add_propdef(obj, pname, obj, prop_flags, initial_value)?;
        Ok(())
    }

    fn find_command_verb_on(
        &mut self,
        oid: Objid,
        pc: &ParsedCommand,
    ) -> Result<Option<VerbInfo>, ObjectError> {
        if !self.valid(oid)? {
            return Ok(None);
        }

        let spec_for_fn = |oid, pco| -> ArgSpec {
            if pco == oid {
                ArgSpec::This
            } else if pco == NOTHING {
                ArgSpec::None
            } else {
                ArgSpec::Any
            }
        };

        let dobj = spec_for_fn(oid, pc.dobj);
        let iobj = spec_for_fn(oid, pc.iobj);

        self.find_command_verb(oid, pc.verb.as_str(), dobj, pc.prep, iobj)
    }

    fn parent_of(&mut self, obj: Objid) -> Result<Objid, ObjectError> {
        let attrs =
            self.object_get_attrs(obj, BitEnum::new_with(ObjAttr::Parent) | ObjAttr::Owner)?;
        // TODO: this masks other (internal?) errors..
        attrs.parent.ok_or(ObjectNotFound(obj))
    }

    fn valid(&mut self, obj: Objid) -> Result<bool, ObjectError> {
        // TODO: this masks other (internal?) errors..
        Ok(self
            .object_get_attrs(obj, BitEnum::new_with(ObjAttr::Parent) | ObjAttr::Owner)
            .is_ok())
    }

    fn names_of(&mut self, obj: Objid) -> Result<(String, Vec<String>), ObjectError> {
        // TODO implement support for aliases.
        let name = self
            .object_get_attrs(obj, BitEnum::from(ObjAttr::Name))?
            .name
            .unwrap();
        Ok((name, vec![]))
    }

    fn commit(&mut self) -> Result<CommitResult, anyhow::Error> {
        match self.get_db().do_commit_tx(&mut self.tx) {
            Ok(_) => Ok(CommitResult::Success),
            Err(relations::RelationError::Conflict) => Ok(CommitResult::ConflictRetry),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn rollback(&mut self) -> Result<(), anyhow::Error> {
        self.get_db().do_rollback_tx(&mut self.tx)?;
        Ok(())
    }
}
