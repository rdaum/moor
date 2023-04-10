use std::collections::Bound::Included;

use anyhow::{anyhow, Error};
use itertools::Itertools;

use crate::db::relations;
use crate::db::relations::Relation;
use crate::db::state::WorldState;
use crate::db::tx::Tx;
use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::props::{Pid, PropAttr, PropAttrs, PropFlag, Propdef, Properties, PropertyInfo};
use crate::model::r#match::VerbArgsSpec;
use crate::model::var::{Objid, Var, NOTHING};
use crate::model::verbs::{VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::Binary;

const MAX_PROP_NAME: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
const MAX_VERB_NAME: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";

// Basic non-transactional, non-persistent in-memory "database" to bootstrap things.

pub struct ImDB {
    next_objid: i64,
    next_pid: i64,
    next_vid: i64,

    // Objects and their attributes
    obj_attr_location: Relation<Objid, Objid>,
    obj_attr_owner: Relation<Objid, Objid>,
    obj_attr_parent: Relation<Objid, Objid>,
    obj_attr_name: Relation<Objid, String>,
    obj_attr_flags: Relation<Objid, BitEnum<ObjFlag>>,

    // Property definitions & properties

    // Property defs are kept in a sorted map keyed by object id, string so that a range query can
    // be performed across the object to retrieve all the property definitions for that object, and
    // so that prefix matching can be performed on the property name.
    // Not guaranteed to be the most efficient structure, but it's simple and it works.
    propdefs: Relation<(Objid, String), Propdef>,

    property_value: Relation<(Objid, Pid), Var>,
    property_location: Relation<(Objid, Pid), Objid>,
    property_owner: Relation<(Objid, Pid), Objid>,
    property_flags: Relation<(Objid, Pid), BitEnum<PropFlag>>,

    // Verbs and their attributes
    verbdefs: Relation<(Objid, String), Vid>,

    verb_names: Relation<Vid, Vec<String>>,
    verb_attr_definer: Relation<Vid, Objid>,
    verb_attr_owner: Relation<Vid, Objid>,
    verb_attr_flags: Relation<Vid, BitEnum<VerbFlag>>,
    verb_attr_args_spec: Relation<Vid, VerbArgsSpec>,
    verb_attr_program: Relation<Vid, Binary>,
}

impl Default for ImDB {
    fn default() -> Self {
        ImDB::new()
    }
}

impl ImDB {
    pub fn new() -> Self {
        Self {
            next_objid: 0,
            next_pid: 0,
            next_vid: 0,
            obj_attr_location: Relation::new_bidirectional(),
            obj_attr_owner: Relation::new_bidirectional(),
            obj_attr_parent: Relation::new_bidirectional(),
            obj_attr_name: Default::default(),
            obj_attr_flags: Default::default(),
            propdefs: Default::default(),
            property_value: Default::default(),
            property_location: Default::default(),
            property_owner: Default::default(),
            property_flags: Default::default(),
            verbdefs: Default::default(),
            verb_names: Default::default(),
            verb_attr_definer: Default::default(),
            verb_attr_owner: Default::default(),
            verb_attr_flags: Default::default(),
            verb_attr_args_spec: Default::default(),
            verb_attr_program: Default::default(),
        }
    }

    pub fn do_begin_tx(&mut self, tx: &mut Tx) -> Result<(), relations::Error> {
        self.obj_attr_location.begin(tx)?;
        self.obj_attr_owner.begin(tx)?;
        self.obj_attr_parent.begin(tx)?;
        self.obj_attr_name.begin(tx)?;
        self.obj_attr_flags.begin(tx)?;
        self.propdefs.begin(tx)?;
        self.property_value.begin(tx)?;
        self.property_location.begin(tx)?;
        self.property_owner.begin(tx)?;
        self.property_flags.begin(tx)?;
        self.verbdefs.begin(tx)?;
        self.verb_names.begin(tx)?;
        self.verb_attr_definer.begin(tx)?;
        self.verb_attr_owner.begin(tx)?;
        self.verb_attr_flags.begin(tx)?;
        self.verb_attr_args_spec.begin(tx)?;
        self.verb_attr_program.begin(tx)?;

        Ok(())
    }

    pub fn do_commit_tx(&mut self, tx: &mut Tx) -> Result<(), relations::Error> {
        self.obj_attr_location.commit(tx)?;
        self.obj_attr_owner.commit(tx)?;
        self.obj_attr_parent.commit(tx)?;
        self.obj_attr_name.commit(tx)?;
        self.obj_attr_flags.commit(tx)?;
        self.propdefs.commit(tx)?;
        self.property_value.commit(tx)?;
        self.property_location.commit(tx)?;
        self.property_owner.commit(tx)?;
        self.property_flags.commit(tx)?;
        self.verbdefs.commit(tx)?;
        self.verb_names.commit(tx)?;
        self.verb_attr_definer.commit(tx)?;
        self.verb_attr_owner.commit(tx)?;
        self.verb_attr_flags.commit(tx)?;
        self.verb_attr_args_spec.commit(tx)?;
        self.verb_attr_program.commit(tx)?;

        Ok(())
    }

    pub fn do_rollback_tx(&mut self, tx: &mut Tx) -> Result<(), relations::Error> {
        self.obj_attr_location.rollback(tx)?;
        self.obj_attr_owner.rollback(tx)?;
        self.obj_attr_parent.rollback(tx)?;
        self.obj_attr_name.rollback(tx)?;
        self.obj_attr_flags.rollback(tx)?;
        self.propdefs.rollback(tx)?;
        self.property_value.rollback(tx)?;
        self.property_location.rollback(tx)?;
        self.property_owner.rollback(tx)?;
        self.property_flags.rollback(tx)?;
        self.verbdefs.rollback(tx)?;
        self.verb_names.rollback(tx)?;
        self.verb_attr_definer.rollback(tx)?;
        self.verb_attr_owner.rollback(tx)?;
        self.verb_attr_flags.rollback(tx)?;
        self.verb_attr_args_spec.rollback(tx)?;
        self.verb_attr_program.rollback(tx)?;

        Ok(())
    }

    pub fn get_object_inheritance_chain(&mut self, tx: &mut Tx, oid: Objid) -> Vec<Objid> {
        if self.obj_attr_flags.seek_for_l_eq(tx, &oid).is_none() {
            return Vec::new();
        }

        // Get the full inheritance hierarchy for 'oid' as a flat list.
        // Start with self, then walk until we hit Objid(-1) or None for parents.
        let mut chain = Vec::new();
        let mut current = oid;
        while current != NOTHING {
            chain.push(current);
            current = self
                .obj_attr_parent
                .seek_for_l_eq(tx, &current)
                .unwrap_or(NOTHING);
        }
        chain
    }

    // Retrieve a property without inheritance search.
    pub fn get_local_property(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        let propkey = (oid, handle);
        let Some(flags) = self.property_flags.seek_for_l_eq(tx, &propkey) else {
            return Ok(None);
        };

        let mut result_attrs = PropAttrs::default();
        if attrs.contains(PropAttr::Value) {
            if let Some(value) = self.property_value.seek_for_l_eq(tx, &propkey) {
                result_attrs.value = Some(value);
            }
        }
        if attrs.contains(PropAttr::Flags) {
            result_attrs.flags = Some(flags);
        }
        if attrs.contains(PropAttr::Owner) {
            if let Some(owner) = self.property_owner.seek_for_l_eq(tx, &propkey) {
                result_attrs.owner = Some(owner);
            }
        }
        if attrs.contains(PropAttr::Location) {
            if let Some(location) = self.property_location.seek_for_l_eq(tx, &propkey) {
                result_attrs.location = Some(location);
            }
        }

        Ok(Some(result_attrs))
    }

    pub fn create_object(
        &mut self,
        tx: &mut Tx,
        oid: Option<Objid>,
        attrs: &ObjAttrs,
    ) -> Result<Objid, Error> {
        let oid = match oid {
            None => {
                let oid = self.next_objid;
                self.next_objid += 1;
                Objid(oid)
            }
            Some(oid) => oid,
        };
        self.obj_attr_name.insert(tx, &oid, &String::new())?;
        self.obj_attr_location.insert(tx, &oid, &NOTHING)?;
        self.obj_attr_owner.insert(tx, &oid, &NOTHING)?;
        self.obj_attr_parent.insert(tx, &oid, &NOTHING)?;

        let noflags: BitEnum<ObjFlag> = BitEnum::new();
        self.obj_attr_flags.insert(tx, &oid, &noflags)?;

        // TODO validate all attributes present.
        self.object_set_attrs(tx, oid, attrs.clone())?;
        Ok(oid)
    }

    pub fn destroy_object(&mut self, tx: &mut Tx, oid: Objid) -> Result<(), Error> {
        if !self.object_valid(tx, oid)? {
            return Err(anyhow!("invalid object"));
        }
        self.obj_attr_parent.remove_for_l(tx, &oid)?;
        self.obj_attr_location.remove_for_l(tx, &oid)?;
        self.obj_attr_flags.remove_for_l(tx, &oid)?;
        self.obj_attr_name.remove_for_l(tx, &oid)?;
        self.obj_attr_owner.remove_for_l(tx, &oid)?;
        Ok(())
    }

    pub fn object_valid(&mut self, tx: &mut Tx, oid: Objid) -> Result<bool, Error> {
        Ok(self.obj_attr_flags.seek_for_l_eq(tx, &oid).is_some())
    }

    pub fn object_get_attrs(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        attributes: BitEnum<ObjAttr>,
    ) -> Result<ObjAttrs, Error> {
        if !self.object_valid(tx, oid)? {
            return Err(anyhow!("invalid object"));
        }
        let mut return_attrs = ObjAttrs::default();
        if attributes.contains(ObjAttr::Owner) {
            return_attrs.owner = self.obj_attr_owner.seek_for_l_eq(tx, &oid);
        }
        if attributes.contains(ObjAttr::Name) {
            return_attrs.name = self.obj_attr_name.seek_for_l_eq(tx, &oid);
        }
        if attributes.contains(ObjAttr::Parent) {
            return_attrs.parent = self.obj_attr_parent.seek_for_l_eq(tx, &oid);
        }
        if attributes.contains(ObjAttr::Location) {
            return_attrs.location = self.obj_attr_location.seek_for_l_eq(tx, &oid);
        }
        if attributes.contains(ObjAttr::Flags) {
            return_attrs.flags = self.obj_attr_flags.seek_for_l_eq(tx, &oid);
        }
        Ok(return_attrs)
    }

    pub fn object_set_attrs(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        attributes: ObjAttrs,
    ) -> Result<(), Error> {
        if !self.object_valid(tx, oid)? {
            return Err(anyhow!("invalid object"));
        }
        if let Some(parent) = attributes.parent {
            self.obj_attr_parent.update_r(tx, &oid, &parent)?;
        }
        if let Some(owner) = attributes.owner {
            self.obj_attr_owner.update_r(tx, &oid, &owner)?;
        }
        if let Some(location) = attributes.location {
            self.obj_attr_location.update_r(tx, &oid, &location)?;
        }
        if let Some(flags) = attributes.flags {
            self.obj_attr_flags.update_r(tx, &oid, &flags)?;
        }
        if let Some(name) = attributes.name {
            self.obj_attr_name.update_r(tx, &oid, &name)?;
        }
        Ok(())
    }

    pub fn object_children(&mut self, tx: &mut Tx, oid: Objid) -> Result<Vec<Objid>, Error> {
        if !self.object_valid(tx, oid)? {
            return Err(anyhow!("invalid object"));
        }
        Ok(self
            .obj_attr_parent
            .seek_for_r_eq(tx, &oid)
            .into_iter()
            .collect())
    }

    pub fn object_contents(&mut self, tx: &mut Tx, oid: Objid) -> Result<Vec<Objid>, Error> {
        if !self.object_valid(tx, oid)? {
            return Err(anyhow!("invalid object"));
        }
        Ok(self
            .obj_attr_location
            .seek_for_r_eq(tx, &oid)
            .into_iter()
            .collect())
    }

    pub fn get_propdef(
        &mut self,
        tx: &mut Tx,
        definer: Objid,
        pname: &str,
    ) -> Result<Propdef, Error> {
        self.propdefs
            .seek_for_l_eq(tx, &(definer, pname.to_string()))
            .ok_or_else(|| anyhow!("no such property definition {} on #{}", pname, definer.0))
    }

    pub fn add_propdef(
        &mut self,
        tx: &mut Tx,

        definer: Objid,
        name: &str,
        owner: Objid,
        flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, Error> {
        let pid = Pid(self.next_pid);
        self.next_pid += 1;
        let pd = Propdef {
            pid,
            definer,
            pname: name.to_string(),
        };
        self.propdefs
            .insert(tx, &(definer, name.to_string().to_lowercase()), &pd)?;

        if let Some(initial_value) = initial_value {
            self.set_property(tx, pid, definer, initial_value, owner, flags)?;
        }

        Ok(pid)
    }

    pub fn rename_propdef(
        &mut self,
        tx: &mut Tx,
        definer: Objid,
        old: &str,
        new: &str,
    ) -> Result<(), Error> {
        match self.propdefs.seek_for_l_eq(tx, &(definer, old.to_string())) {
            None => return Err(anyhow!("property does not exist")),
            Some(pd) => {
                self.propdefs
                    .remove_for_l(tx, &(definer, old.to_string()))?;
                let mut new_pd = pd;
                new_pd.pname = new.to_string();
                self.propdefs
                    .insert(tx, &(definer, new.to_string()), &new_pd)?;
            }
        }
        Ok(())
    }

    pub fn delete_propdef(
        &mut self,
        tx: &mut Tx,
        definer: Objid,
        pname: &str,
    ) -> Result<(), Error> {
        self.propdefs
            .remove_for_l(tx, &(definer, pname.to_string()))?;
        Ok(())
    }

    pub fn count_propdefs(&mut self, tx: &mut Tx, definer: Objid) -> Result<usize, Error> {
        let start = (definer, String::new());
        let end = (definer, MAX_PROP_NAME.to_string());
        let range = self
            .propdefs
            .range_for_l_eq(tx, (Included(&start), Included(&end)));
        Ok(range.len())
    }

    pub fn get_propdefs(&mut self, tx: &mut Tx, definer: Objid) -> Result<Vec<Propdef>, Error> {
        let start = (definer, String::new());
        let end = (definer, MAX_PROP_NAME.to_string());
        let range = self
            .propdefs
            .range_for_l_eq(tx, (Included(&start), Included(&end)));
        Ok(range.iter().map(|(_, pd)| pd.clone()).collect())
    }

    pub fn find_property(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        name: &str,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        let self_and_parents = self.get_object_inheritance_chain(tx, oid);

        // Look for the property definition on self and then all the way up the parents, stopping
        // at the first match.
        let propdef = self_and_parents
            .iter()
            .filter_map(|&oid| self.propdefs.seek_for_l_eq(tx, &(oid, name.to_string())))
            .next()
            .ok_or_else(|| anyhow!("no such property"))?;

        // Then use the Pid from that to again look at self and all the way up the parents for the
        let pid = propdef.pid;
        for oid in self_and_parents {
            if let Some(propattrs) = self.get_local_property(tx, oid, pid, attrs)? {
                return Ok(Some(PropertyInfo {
                    pid,
                    attrs: propattrs,
                }));
            }
        }

        Ok(None)
    }

    pub fn get_property(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        handle: Pid,
        attrs: BitEnum<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        let self_and_parents = self.get_object_inheritance_chain(tx, oid);
        for oid in self_and_parents {
            let propattrs = self.get_local_property(tx, oid, handle, attrs)?;
            if propattrs.is_some() {
                return Ok(propattrs);
            }
        }

        Ok(None)
    }

    pub fn set_property(
        &mut self,
        tx: &mut Tx,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: BitEnum<PropFlag>,
    ) -> Result<(), Error> {
        let propkey = (location, handle);
        self.property_value.insert(tx, &propkey, &value)?;
        self.property_flags.insert(tx, &propkey, &flags)?;
        self.property_owner.insert(tx, &propkey, &owner)?;
        self.property_location.insert(tx, &propkey, &location)?;

        Ok(())
    }

    pub fn add_verb(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: BitEnum<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Binary,
    ) -> Result<VerbInfo, Error> {
        let vid = Vid(self.next_vid);
        self.next_vid += 1;

        for name in names.clone() {
            self.verbdefs.insert(tx, &(oid, name.to_string()), &vid)?;
        }

        self.verb_attr_definer.insert(tx, &vid, &oid)?;
        self.verb_attr_owner.insert(tx, &vid, &owner)?;
        self.verb_attr_flags.insert(tx, &vid, &flags)?;
        self.verb_attr_program.insert(tx, &vid, &program)?;
        self.verb_attr_args_spec.insert(tx, &vid, &arg_spec)?;
        let name_set: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        self.verb_names.insert(tx, &vid, &name_set)?;

        let vi = VerbInfo {
            vid,
            names: names.into_iter().map(|s| s.to_string()).collect(),
            attrs: VerbAttrs {
                definer: Some(oid),
                owner: Some(owner),
                flags: Some(flags),
                args_spec: Some(arg_spec),
                program: Some(program),
            },
        };
        Ok(vi)
    }

    pub fn get_verbs(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Vec<VerbInfo>, Error> {
        let obj_verbs = self.verbdefs.range_for_l_eq(
            tx,
            (
                Included(&(oid, String::new())),
                Included(&(oid, MAX_VERB_NAME.to_string())),
            ),
        );

        let verbs_by_vid = obj_verbs.iter().group_by(|v| v.1);

        let mut verbs = vec![];
        for (vid, verb) in &verbs_by_vid {
            let v = self.get_verb(tx, vid, attrs)?;
            let names: Vec<_> = verb.map(|verb| verb.0 .1.clone()).collect();
            verbs.push(VerbInfo {
                vid,
                names,
                attrs: v.attrs,
            })
        }

        Ok(verbs)
    }

    pub fn get_verb(
        &mut self,
        tx: &mut Tx,
        vid: Vid,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<VerbInfo, Error> {
        if self.verb_attr_args_spec.seek_for_l_eq(tx, &vid).is_none() {
            return Err(anyhow!("no such verb"));
        }

        let names = self.verb_names.seek_for_l_eq(tx, &vid).unwrap();

        let mut return_attrs = VerbAttrs {
            definer: None,
            owner: None,
            flags: None,
            args_spec: None,
            program: None,
        };
        if attrs.contains(VerbAttr::Definer) {
            return_attrs.definer = self.verb_attr_definer.seek_for_l_eq(tx, &vid);
        }
        if attrs.contains(VerbAttr::Owner) {
            return_attrs.owner = self.verb_attr_owner.seek_for_l_eq(tx, &vid);
        }
        if attrs.contains(VerbAttr::Flags) {
            return_attrs.flags = self.verb_attr_flags.seek_for_l_eq(tx, &vid);
        }
        if attrs.contains(VerbAttr::ArgsSpec) {
            return_attrs.args_spec = self.verb_attr_args_spec.seek_for_l_eq(tx, &vid);
        }
        if attrs.contains(VerbAttr::Program) {
            return_attrs.program = self.verb_attr_program.seek_for_l_eq(tx, &vid);
        }

        Ok(VerbInfo {
            vid,
            names,
            attrs: return_attrs,
        })
    }

    pub fn update_verb(&self, _tx: &mut Tx, _vid: Vid, _attrs: VerbAttrs) -> Result<(), Error> {
        // Updating names is going to be complicated! Rewriting the oid,name index to remove the
        // old names, then re-establishing them...

        todo!()
    }

    pub fn find_command_verb(
        &self,
        _tx: &mut Tx,

        _oid: Objid,
        _verb: &str,
        _arg_spec: VerbArgsSpec,
        _attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }

    pub fn find_callable_verb(
        &mut self,
        tx: &mut Tx,
        oid: Objid,
        verb: &str,
        attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        let parent_chain = self.get_object_inheritance_chain(tx, oid);
        for parent in parent_chain {
            let vid = self.verbdefs.seek_for_l_eq(tx, &(parent, verb.to_string()));
            if let Some(vid) = vid {
                let vi = self.get_verb(tx, vid, attrs)?;
                return Ok(Some(vi));
            }
        }
        Ok(None)
    }

    pub fn find_indexed_verb(
        &self,
        _tx: &mut Tx,

        _oid: Objid,
        _index: usize,
        _attrs: BitEnum<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }

    pub fn property_allows(
        &self,
        _tx: &mut Tx,
        _check_flags: BitEnum<PropFlag>,
        _player: Objid,
        _player_flags: BitEnum<ObjFlag>,
        _prop_flags: BitEnum<PropFlag>,
        _prop_owner: Objid,
    ) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::db::inmem_db::ImDB;
    use crate::db::tx::Tx;
    use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
    use crate::model::props::{PropAttr, PropDefs, PropFlag, Propdef, Properties};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::{Objid, Var};
    use crate::model::verbs::{VerbAttr, VerbFlag, Verbs};
    use crate::util::bitenum::BitEnum;
    use crate::vm::opcode::Binary;

    #[test]
    fn object_create_check_delete() {
        let mut s = ImDB::default();
        let mut tx = Tx::new(0, 0);

        let o = s.create_object(&mut tx, None, &ObjAttrs::new()).unwrap();
        assert!(s.object_valid(&mut tx, o).unwrap());
        s.destroy_object(&mut tx, o).unwrap();
        assert!(!s.object_valid(&mut tx, o).unwrap());

        s.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn object_check_children_contents() {
        let mut s = ImDB::default();
        let mut tx = Tx::new(0, 0);

        let o1 = s
            .create_object(&mut tx, None, ObjAttrs::new().name("test"))
            .unwrap();
        let o2 = s
            .create_object(
                &mut tx,
                None,
                ObjAttrs::new().name("test2").location(o1).parent(o1),
            )
            .unwrap();
        let o3 = s
            .create_object(
                &mut tx,
                None,
                ObjAttrs::new().name("test3").location(o1).parent(o1),
            )
            .unwrap();

        let mut children = s.object_children(&mut tx, o1).unwrap();
        assert_eq!(children.sort(), vec![o2, o3].sort());

        let mut contents = s.object_contents(&mut tx, o1).unwrap();
        assert_eq!(contents.sort(), vec![o2, o3].sort());

        s.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn object_create_set_get_attrs() {
        let mut s = ImDB::default();
        let mut tx = Tx::new(0, 0);

        let o = s
            .create_object(
                &mut tx,
                None,
                ObjAttrs::new()
                    .name("test")
                    .flags(BitEnum::new_with(ObjFlag::Write) | ObjFlag::Read),
            )
            .unwrap();

        let attrs = s
            .object_get_attrs(
                &mut tx,
                o,
                BitEnum::new_with(ObjAttr::Flags) | ObjAttr::Name,
            )
            .unwrap();

        assert_eq!(attrs.name.unwrap(), "test");
        assert!(attrs.flags.unwrap().contains(ObjFlag::Write));

        s.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn test_inheritance_chain() {
        let mut odb = ImDB::default();
        let mut tx = Tx::new(0, 0);

        // Create objects and establish parent-child relationship
        let o1 = odb
            .create_object(&mut tx, Some(Objid(1)), ObjAttrs::new().name("o1"))
            .unwrap();
        let o2 = odb
            .create_object(
                &mut tx,
                Some(Objid(2)),
                ObjAttrs::new().name("o2").parent(o1),
            )
            .unwrap();
        let _o3 = odb
            .create_object(
                &mut tx,
                Some(Objid(3)),
                ObjAttrs::new().name("o3").parent(o2),
            )
            .unwrap();
        let _o4 = odb
            .create_object(
                &mut tx,
                Some(Objid(4)),
                ObjAttrs::new().name("o4").parent(o2),
            )
            .unwrap();
        let o5 = odb
            .create_object(
                &mut tx,
                Some(Objid(5)),
                ObjAttrs::new().name("o5").parent(o1),
            )
            .unwrap();
        let o6 = odb
            .create_object(
                &mut tx,
                Some(Objid(6)),
                ObjAttrs::new().name("o6").parent(o5),
            )
            .unwrap();

        // Test inheritance chain for o6
        let inheritance_chain = odb.get_object_inheritance_chain(&mut tx, o6);
        assert_eq!(inheritance_chain, vec![Objid(6), Objid(5), Objid(1)]);

        // Test inheritance chain for o2
        let inheritance_chain = odb.get_object_inheritance_chain(&mut tx, o2);
        assert_eq!(inheritance_chain, vec![Objid(2), Objid(1)]);

        // Test inheritance chain for o1
        let inheritance_chain = odb.get_object_inheritance_chain(&mut tx, o1);
        assert_eq!(inheritance_chain, vec![Objid(1)]);

        // Test inheritance chain for non-existent object
        let inheritance_chain = odb.get_object_inheritance_chain(&mut tx, Objid(7));
        assert_eq!(inheritance_chain, Vec::<Objid>::new());

        // Test object_children for o1
        let mut children = odb.object_children(&mut tx, o1).unwrap();
        assert_eq!(children.sort(), vec![Objid(2), Objid(5)].sort());

        // Test object_children for o2
        let mut children = odb.object_children(&mut tx, o2).unwrap();
        assert_eq!(children.sort(), vec![Objid(3), Objid(4)].sort());

        // Test object_children for non-existent object
        let children = odb.object_children(&mut tx, Objid(7));
        assert!(children.is_err());

        odb.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn test_propdefs() {
        let mut odb = ImDB::default();
        let mut tx = Tx::new(0, 0);

        // Add some property definitions.
        let pid1 = odb
            .add_propdef(
                &mut tx,
                Objid(1),
                "color",
                Objid(1),
                BitEnum::new_with(PropFlag::Read),
                None,
            )
            .unwrap();
        let pid2 = odb
            .add_propdef(
                &mut tx,
                Objid(1),
                "size",
                Objid(2),
                BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
                Some(Var::Int(42)),
            )
            .unwrap();

        // Get a property definition by its name.
        let def1 = odb.get_propdef(&mut tx, Objid(1), "color").unwrap();
        assert_eq!(def1.pid, pid1);
        assert_eq!(def1.definer, Objid(1));
        assert_eq!(def1.pname, "color");

        // Rename a property.
        odb.rename_propdef(&mut tx, Objid(1), "color", "shade")
            .unwrap();
        let def2 = odb.get_propdef(&mut tx, Objid(1), "shade").unwrap();
        assert_eq!(def2.pid, pid1);
        assert_eq!(def2.definer, Objid(1));
        assert_eq!(def2.pname, "shade");

        // Get all property definitions on an object.
        let defs = odb.get_propdefs(&mut tx, Objid(1)).unwrap();
        assert_eq!(defs.len(), 2);
        assert!(defs.contains(&def2));
        assert!(defs.contains(&Propdef {
            pid: pid2,
            definer: Objid(1),
            pname: "size".to_owned(),
        }));

        // Delete a property definition.
        odb.delete_propdef(&mut tx, Objid(1), "size").unwrap();
        let defs = odb.get_propdefs(&mut tx, Objid(1)).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0], def2);

        // Count the number of property definitions on an object.
        let count = odb.count_propdefs(&mut tx, Objid(1)).unwrap();
        assert_eq!(count, 1);

        odb.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn property_inheritance() {
        let mut s = ImDB::default();
        let mut tx = Tx::new(0, 0);

        let parent = s.create_object(&mut tx, None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(&mut tx, None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let pid = s
            .add_propdef(
                &mut tx,
                parent,
                "test",
                parent,
                BitEnum::new_with(PropFlag::Chown) | PropFlag::Read,
                Some(Var::Str(String::from("testing"))),
            )
            .unwrap();

        let pds = s.get_propdefs(&mut tx, parent).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, parent);
        assert_eq!(pds[0].pid, pid, "test");

        // Verify initially that we get the value all the way from root.
        let v = s
            .get_property(
                &mut tx,
                child2,
                pid,
                BitEnum::new_with(PropAttr::Value) | PropAttr::Location,
            )
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(parent));

        // Set it on the intermediate child...
        s.set_property(
            &mut tx,
            pid,
            child1,
            Var::Str(String::from("testing")),
            parent,
            BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(
                &mut tx,
                child2,
                pid,
                BitEnum::new_with(PropAttr::Value) | PropAttr::Location,
            )
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child1));

        // Finally set it on the last child...
        s.set_property(
            &mut tx,
            pid,
            child2,
            Var::Str(String::from("testing")),
            parent,
            BitEnum::new_with(PropFlag::Read) | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(
                &mut tx,
                child2,
                pid,
                BitEnum::new_with(PropAttr::Value) | PropAttr::Location,
            )
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child2));

        // Finally, use the name to look it up instead of the pid
        let v = s
            .find_property(
                &mut tx,
                child2,
                "test",
                BitEnum::new_with(PropAttr::Value) | PropAttr::Location,
            )
            .unwrap()
            .unwrap();
        assert_eq!(v.attrs.location, Some(child2));
        // And verify we don't get it from other root or from its child
        let v = s
            .get_property(
                &mut tx,
                other_root,
                pid,
                BitEnum::new_with(PropAttr::Value) | PropAttr::Location,
            )
            .unwrap();
        assert!(v.is_none());

        s.do_commit_tx(&mut tx).unwrap();
    }

    #[test]
    fn verb_inheritance() {
        let mut s = ImDB::default();
        let mut tx = Tx::new(0, 0);

        let parent = s.create_object(&mut tx, None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(&mut tx, None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(&mut tx, None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let thisnonethis = VerbArgsSpec {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        };
        let _vinfo = s
            .add_verb(
                &mut tx,
                parent,
                vec!["look_down", "look_up"],
                parent,
                BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Read,
                thisnonethis,
                Binary::default(),
            )
            .unwrap();

        let verbs = s
            .get_verbs(
                &mut tx,
                parent,
                BitEnum::new_with(VerbAttr::Definer)
                    | VerbAttr::Owner
                    | VerbAttr::Flags
                    | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert_eq!(verbs.len(), 1);
        assert_eq!(verbs[0].attrs.definer.unwrap(), parent);
        assert_eq!(verbs[0].attrs.args_spec.unwrap(), thisnonethis);
        assert_eq!(verbs[0].attrs.owner.unwrap(), parent);
        assert_eq!(verbs[0].names.len(), 2);

        // Verify initially that we get the value all the way from root.
        let v = s
            .find_callable_verb(
                &mut tx,
                child2,
                "look_up",
                BitEnum::new_with(VerbAttr::Definer) | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), parent);

        // Set it on the intermediate child...
        let _vinfo = s
            .add_verb(
                &mut tx,
                child1,
                vec!["look_down", "look_up"],
                parent,
                BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Read,
                thisnonethis,
                Binary::default(),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                &mut tx,
                child2,
                "look_up",
                BitEnum::new_with(VerbAttr::Definer) | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child1);

        // Finally set it on the last child...
        let _vinfo = s
            .add_verb(
                &mut tx,
                child2,
                vec!["look_down", "look_up"],
                parent,
                BitEnum::new_with(VerbFlag::Exec) | VerbFlag::Read,
                thisnonethis,
                Binary::default(),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                &mut tx,
                child2,
                "look_up",
                BitEnum::new_with(VerbAttr::Definer) | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child2);

        // And verify we don't get it from other root or from its child
        let v = s
            .find_callable_verb(
                &mut tx,
                other_root,
                "look_up",
                BitEnum::new_with(VerbAttr::Definer) | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_none());

        s.do_commit_tx(&mut tx).unwrap();
    }
}
