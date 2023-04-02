use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::permissions::Permissions;
use crate::model::props::{
    Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties, PropertyInfo,
};
use crate::model::r#match::VerbArgsSpec;

use crate::model::var::{Objid, Var, NOTHING};
use crate::model::verbs::{Program, VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::model::ObjDB;
use anyhow::{anyhow, Error};
use enumset::EnumSet;
use std::collections::btree_map::Entry;
use std::collections::Bound::Included;
use std::collections::{BTreeMap, HashMap, HashSet};



use itertools::Itertools;
use crate::db::state::{WorldState};


const MAX_PROP_NAME: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
const MAX_VERB_NAME: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";

// Basic non-transactional, non-persistent in-memory "database" to bootstrap things.

pub struct ImDB {
    next_objid: i64,
    next_pid: i64,
    next_vid: i64,

    // Objects and their attributes
    objects: HashSet<Objid>,
    obj_attr_location: HashMap<Objid, Objid>,
    obj_attr_owner: HashMap<Objid, Objid>,
    obj_attr_parent: HashMap<Objid, Objid>,
    obj_attr_name: HashMap<Objid, String>,
    obj_attr_flags: HashMap<Objid, EnumSet<ObjFlag>>,

    // Derived
    obj_contents: HashMap<Objid, HashSet<Objid>>,
    obj_children: HashMap<Objid, HashSet<Objid>>,

    // Property definitions & properties

    // Property defs are kept in a sorted map keyed by object id, string so that a range query can
    // be performed across the object to retrieve all the property definitions for that object, and
    // so that prefix matching can be performed on the property name.
    // Not guaranteed to be the most efficient structure, but it's simple and it works.
    propdefs: BTreeMap<(Objid, String), Propdef>,

    properties: HashSet<(Objid, Pid)>,
    property_value: HashMap<(Objid, Pid), Var>,
    property_location: HashMap<(Objid, Pid), Objid>,
    property_owner: HashMap<(Objid, Pid), Objid>,
    property_flags: HashMap<(Objid, Pid), EnumSet<PropFlag>>,

    // Verbs and their attributes
    verbdefs: BTreeMap<(Objid, String), Vid>,

    verbs: HashSet<Vid>,
    verb_names: HashMap<Vid, HashSet<String>>,
    verb_attr_definer: HashMap<Vid, Objid>,
    verb_attr_owner: HashMap<Vid, Objid>,
    verb_attr_flags: HashMap<Vid, EnumSet<VerbFlag>>,
    verb_attr_args_spec: HashMap<Vid, VerbArgsSpec>,
    verb_attr_program: HashMap<Vid, Program>,
}

impl ImDB {
    pub fn new() -> Self {
        Self {
            next_objid: 0,
            next_pid: 0,
            next_vid: 0,
            objects: Default::default(),
            obj_attr_location: Default::default(),
            obj_attr_owner: Default::default(),
            obj_attr_parent: Default::default(),
            obj_attr_name: Default::default(),
            obj_attr_flags: Default::default(),
            obj_contents: Default::default(),
            obj_children: Default::default(),
            propdefs: Default::default(),
            properties: Default::default(),
            property_value: Default::default(),
            property_location: Default::default(),
            property_owner: Default::default(),
            property_flags: Default::default(),
            verbdefs: Default::default(),
            verbs: Default::default(),
            verb_names: Default::default(),
            verb_attr_definer: Default::default(),
            verb_attr_owner: Default::default(),
            verb_attr_flags: Default::default(),
            verb_attr_args_spec: Default::default(),
            verb_attr_program: Default::default(),
        }
    }

    pub fn get_object_inheritance_chain(&self, oid: Objid) -> Vec<Objid> {
        if !self.objects.contains(&oid) {
            return Vec::new();
        }
        // Get the full inheritance hierarchy for 'oid' as a flat list.
        // Start with self, then walk until we hit Objid(-1) or None for parents.
        let mut chain = Vec::new();
        let mut current = oid;
        while current != NOTHING {
            chain.push(current);
            current = *self
                .obj_attr_parent
                .get(&current)
                .unwrap_or(&NOTHING);
        }
        chain
    }

    // Retrieve a property without inheritance search.
    fn retrieve_property(
        &self,
        oid: Objid,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        let propkey = (oid, handle);
        if !self.properties.contains(&propkey) {
            return Ok(None);
        }

        let mut result_attrs = PropAttrs::default();
        if attrs.contains(PropAttr::Value) {
            if let Some(value) = self.property_value.get(&propkey) {
                result_attrs.value = Some(value.clone());
            }
        }
        if attrs.contains(PropAttr::Flags) {
            if let Some(flags) = self.property_flags.get(&propkey) {
                result_attrs.flags = Some(*flags);
            }
        }
        if attrs.contains(PropAttr::Owner) {
            if let Some(owner) = self.property_owner.get(&propkey) {
                result_attrs.owner = Some(*owner);
            }
        }
        if attrs.contains(PropAttr::Location) {
            if let Some(location) = self.property_location.get(&propkey) {
                result_attrs.location = Some(*location);
            }
        }

        Ok(Some(result_attrs))
    }
}

impl Default for ImDB {
    fn default() -> Self {
        ImDB::new()
    }
}

impl Objects for ImDB {
    fn create_object(&mut self, oid: Option<Objid>, attrs: &ObjAttrs) -> Result<Objid, Error> {
        let oid = match oid {
            None => {
                let oid = self.next_objid;
                self.next_objid += 1;
                Objid(oid)
            }
            Some(oid) => oid,
        };
        self.objects.insert(oid);
        self.obj_attr_name.insert(oid, String::new());
        self.obj_attr_location.insert(oid, NOTHING);
        if attrs.location.is_some() {
            self.obj_contents
                .entry(attrs.location.unwrap())
                .and_modify(|c| {
                    c.insert(oid);
                })
                .or_default();
        }
        self.obj_attr_owner.insert(oid, NOTHING);
        self.obj_attr_parent.insert(oid, NOTHING);
        if attrs.parent.is_some() {
            self.obj_children
                .entry(attrs.parent.unwrap())
                .and_modify(|c| {
                    c.insert(oid);
                })
                .or_default();
        }
        let noflags: EnumSet<ObjFlag> = EnumSet::new();
        self.obj_attr_flags.insert(oid, noflags);

        // TODO validate all attributes present.
        self.object_set_attrs(oid, attrs.clone())?;
        Ok(oid)
    }

    fn destroy_object(&mut self, oid: Objid) -> Result<(), Error> {
        match self.objects.remove(&oid) {
            false => Err(anyhow!("invalid object")),
            true => {
                if let Some(parent) = self.obj_attr_parent.remove(&oid) {
                    // remove from parent's children
                    if let Some(parents_children) = self.obj_children.get_mut(&parent) {
                        parents_children.remove(&parent);
                    }
                }
                if let Some(location) = self.obj_attr_location.remove(&oid) {
                    // remove from location's contents
                    if let Some(location_contents) = self.obj_contents.get_mut(&oid) {
                        location_contents.remove(&location);
                    }
                }
                self.obj_attr_flags.remove(&oid);
                self.obj_attr_name.remove(&oid);
                self.obj_attr_owner.remove(&oid);
                Ok(())
            }
        }
    }

    fn object_valid(&self, oid: Objid) -> Result<bool, Error> {
        Ok(self.objects.contains(&oid))
    }

    fn object_get_attrs(
        &mut self,
        oid: Objid,
        attributes: EnumSet<ObjAttr>,
    ) -> Result<ObjAttrs, Error> {
        if !self.object_valid(oid)? {
            return Err(anyhow!("invalid object"));
        }
        let mut return_attrs = ObjAttrs::default();
        for a in attributes {
            match a {
                ObjAttr::Owner => return_attrs.owner = self.obj_attr_owner.get(&oid).cloned(),
                ObjAttr::Name => return_attrs.name = self.obj_attr_name.get(&oid).cloned(),
                ObjAttr::Parent => return_attrs.parent = self.obj_attr_parent.get(&oid).cloned(),
                ObjAttr::Location => {
                    return_attrs.location = self.obj_attr_location.get(&oid).cloned()
                }
                ObjAttr::Flags => return_attrs.flags = self.obj_attr_flags.get(&oid).cloned(),
            }
        }
        Ok(return_attrs)
    }

    fn object_set_attrs(&mut self, oid: Objid, attributes: ObjAttrs) -> Result<(), Error> {
        if !self.object_valid(oid)? {
            return Err(anyhow!("invalid object"));
        }
        if let Some(parent) = attributes.parent {
            if let Some(old_parent) = self.obj_attr_parent.insert(oid, parent) {
                if let Some(ch) = self.obj_children.get_mut(&old_parent) {
                    ch.remove(&oid);
                }
            }
            self.obj_children
                .entry(parent)
                .and_modify(|c| {
                    c.insert(oid);
                })
                .or_default();
        }
        if let Some(owner) = attributes.owner {
            self.obj_attr_owner.insert(oid, owner);
        }
        if let Some(location) = attributes.location {
            if let Some(old_location) = self.obj_attr_location.insert(oid, location) {
                if let Some(con) = self.obj_contents.get_mut(&old_location) {
                    con.remove(&oid);
                }
            }
            self.obj_contents
                .entry(location)
                .and_modify(|c| {
                    c.insert(oid);
                })
                .or_default();
        }
        if let Some(flags) = attributes.flags {
            self.obj_attr_flags.insert(oid, flags);
        }
        if let Some(name) = attributes.name {
            self.obj_attr_name.insert(oid, name);
        }
        Ok(())
    }

    fn object_children(&self, oid: Objid) -> Result<Vec<Objid>, Error> {
        if !self.object_valid(oid)? {
            return Err(anyhow!("invalid object"));
        }
        match self.obj_children.get(&oid) {
            None => Ok(vec![]),
            Some(c) => Ok(c.iter().cloned().collect()),
        }
    }

    fn object_contents(&self, oid: Objid) -> Result<Vec<Objid>, Error> {
        if !self.object_valid(oid)? {
            return Err(anyhow!("invalid object"));
        }
        match self.obj_contents.get(&oid) {
            None => Ok(vec![]),
            Some(c) => Ok(c.iter().cloned().collect()),
        }
    }
}

impl PropDefs for ImDB {
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, Error> {
        self.propdefs
            .get(&(definer, pname.to_string()))
            .cloned()
            .ok_or_else(|| anyhow!("no such property definition {} on #{}", pname, definer.0))
    }

    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, Error> {
        match self.propdefs.get(&(definer, name.to_string())) {
            None => {
                let pid = Pid(self.next_pid);
                self.next_pid += 1;
                let pd = Propdef {
                    pid,
                    definer,
                    pname: name.to_string(),
                };
                self.propdefs
                    .insert((definer, name.to_string().to_lowercase()), pd);

                if let Some(initial_value) = initial_value {
                    self.set_property(pid, definer, initial_value, owner, flags)?;
                }

                Ok(pid)
            }
            Some(_) => Err(anyhow!("property already defined")),
        }
    }

    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), Error> {
        match self.propdefs.entry((definer, old.to_string())) {
            Entry::Occupied(e) => {
                let mut pd = e.remove();
                pd.pname = new.to_string();
                self.propdefs.insert((definer, new.to_string()), pd);
                Ok(())
            }
            Entry::Vacant(_) => Err(anyhow!("no such property")),
        }
    }

    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), Error> {
        match self.propdefs.entry((definer, pname.to_string())) {
            Entry::Occupied(e) => {
                e.remove();
                Ok(())
            }
            Entry::Vacant(_) => Err(anyhow!("no such property")),
        }
    }

    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, Error> {
        let start = (definer, String::new());
        let end = (definer, MAX_PROP_NAME.to_string());
        let range = self.propdefs.range((Included(&start), Included(&end)));
        Ok(range.count())
    }

    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, Error> {
        let start = (definer, String::new());
        let end = (definer, MAX_PROP_NAME.to_string());
        let range = self.propdefs.range((Included(&start), Included(&end)));
        Ok(range.map(|(_, pd)| pd.clone()).collect())
    }
}

// TODO all of MOO's wack "clear" property bits, etc.
impl Properties for ImDB {
    fn find_property(
        &self,
        oid: Objid,
        name: &str,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        let self_and_parents = self.get_object_inheritance_chain(oid);

        // Look for the property definition on self and then all the way up the parents, stopping
        // at the first match.
        let propdef = self_and_parents
            .iter()
            .filter_map(|&oid| self.propdefs.get(&(oid, name.to_string())))
            .next()
            .ok_or_else(|| anyhow!("no such property"))?;

        // Then use the Pid from that to again look at self and all the way up the parents for the
        let pid = propdef.pid;
        for oid in self_and_parents {
            if let Some(propattrs) = self.retrieve_property(oid, pid, attrs)? {
                return Ok(Some(PropertyInfo {
                    pid,
                    attrs: propattrs,
                }));
            }
        }

        Ok(None)
    }

    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        let self_and_parents = self.get_object_inheritance_chain(oid);
        for oid in self_and_parents {
            let propattrs = self.retrieve_property(oid, handle, attrs)?;
            if propattrs.is_some() {
                return Ok(propattrs);
            }
        }

        Ok(None)
    }

    fn set_property(
        &mut self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), Error> {
        let propkey = (location, handle);
        self.properties.insert(propkey);
        self.property_value.insert(propkey, value);
        self.property_flags.insert(propkey, flags);
        self.property_owner.insert(propkey, owner);
        self.property_location.insert(propkey, location);

        Ok(())
    }
}

impl Verbs for ImDB {
    fn add_verb(
        &mut self,
        oid: Objid,
        names: Vec<&str>,
        owner: Objid,
        flags: EnumSet<VerbFlag>,
        arg_spec: VerbArgsSpec,
        program: Program,
    ) -> Result<VerbInfo, Error> {
        let vid = Vid(self.next_vid);
        self.next_vid += 1;

        for name in names.clone() {
            self.verbdefs.insert((oid, name.to_string()), vid);
        }

        self.verbs.insert(vid);
        self.verb_attr_definer.insert(vid, oid);
        self.verb_attr_owner.insert(vid, owner);
        self.verb_attr_flags.insert(vid, flags);
        self.verb_attr_program.insert(vid, program.clone());
        self.verb_attr_args_spec.insert(vid, arg_spec);
        let name_set = names.clone().into_iter().map(|s| s.to_string()).collect();
        self.verb_names.insert(vid, name_set);

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

    fn get_verbs(&self, oid: Objid, attrs: EnumSet<VerbAttr>) -> Result<Vec<VerbInfo>, Error> {
        let obj_verbs = self.verbdefs
            .range((Included(&(oid, String::new())), Included(&(oid, MAX_VERB_NAME.to_string()))));

        let verbs_by_vid = obj_verbs.group_by(|v|v.1);

        let mut verbs = vec![];
        for (vid, verb) in &verbs_by_vid {
            let v = self.get_verb(*vid, attrs)?;
            let names :  Vec<_> = verb.map(|verb|verb.0.1.clone()).collect();
            verbs.push(VerbInfo {
                vid: *vid,
                names,
                attrs: v.attrs
            })
        }

        Ok(verbs)
    }

    fn get_verb(&self, vid: Vid, attrs: EnumSet<VerbAttr>) -> Result<VerbInfo, Error> {
        if !self.verbs.contains(&vid) {
            return Err(anyhow!("no such verb"));
        }

        let names = self.verb_names.get(&vid).unwrap().iter().cloned().collect();

        let mut return_attrs = VerbAttrs {
            definer: None,
            owner: None,
            flags: None,
            args_spec: None,
            program: None,
        };
        if attrs.contains(VerbAttr::Definer) {
            return_attrs.definer = self.verb_attr_definer.get(&vid).cloned();
        }
        if attrs.contains(VerbAttr::Owner) {
            return_attrs.owner = self.verb_attr_owner.get(&vid).cloned();
        }
        if attrs.contains(VerbAttr::Flags) {
            return_attrs.flags = self.verb_attr_flags.get(&vid).cloned();
        }
        if attrs.contains(VerbAttr::ArgsSpec) {
            return_attrs.args_spec = self.verb_attr_args_spec.get(&vid).cloned();
        }
        if attrs.contains(VerbAttr::Program) {
            return_attrs.program = self.verb_attr_program.get(&vid).cloned();
        }

        Ok(VerbInfo {
            vid,
            names,
            attrs: return_attrs,
        })

    }

    fn update_verb(&self, _vid: Vid, _attrs: VerbAttrs) -> Result<(), Error> {
        // Updating names is going to be complicated! Rewriting the oid,name index to remove the
        // old names, then re-establishing them...

        todo!()
    }

    fn find_command_verb(
        &self,
        _oid: Objid,
        _verb: &str,
        _arg_spec: VerbArgsSpec,
        _attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }

    fn find_callable_verb(
        &self,
        oid: Objid,
        verb: &str,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        let parent_chain = self.get_object_inheritance_chain(oid);
        for parent in parent_chain {
            let vid = self.verbdefs.get(&(parent, verb.to_string()));
            if let Some(vid) = vid {
                let vi = self.get_verb(*vid, attrs)?;
                return Ok(Some(vi));
            }
        }
        Ok(None)
    }

    fn find_indexed_verb(
        &self,
        _oid: Objid,
        _index: usize,
        _attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }
}

impl Permissions for ImDB {
    fn property_allows(
        &self,
        _check_flags: EnumSet<PropFlag>,
        _player: Objid,
        _player_flags: EnumSet<ObjFlag>,
        _prop_flags: EnumSet<PropFlag>,
        _prop_owner: Objid,
    ) -> bool {
        todo!()
    }
}

impl ObjDB for ImDB {
    fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn commit(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), Error> {
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use crate::db::inmem_db::ImDB;
    use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
    use crate::model::props::{PropAttr, PropDefs, PropFlag, Propdef, Properties};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::{Objid, Var};
    use crate::model::verbs::{Program, VerbAttr, VerbFlag, Verbs};
    use crate::model::ObjDB;
    use enumset::enum_set;

    #[test]
    fn object_create_check_delete() {
        let mut s = ImDB::default();

        let o = s.create_object(None, &ObjAttrs::new()).unwrap();
        assert!(s.object_valid(o).unwrap());
        s.destroy_object(o).unwrap();
        assert!(!s.object_valid(o).unwrap());
        s.commit().unwrap();
    }

    #[test]
    fn object_check_children_contents() {
        let mut s = ImDB::default();

        let o1 = s.create_object(None, ObjAttrs::new().name("test")).unwrap();
        let o2 = s
            .create_object(None, ObjAttrs::new().name("test2").location(o1).parent(o1))
            .unwrap();
        let o3 = s
            .create_object(None, ObjAttrs::new().name("test3").location(o1).parent(o1))
            .unwrap();

        let mut children = s.object_children(o1).unwrap();
        assert_eq!(children.sort(), vec![o2, o3].sort());

        let mut contents = s.object_contents(o1).unwrap();
        assert_eq!(contents.sort(), vec![o2, o3].sort());

        s.commit().unwrap();
    }
    #[test]
    fn object_create_set_get_attrs() {
        let mut s = ImDB::default();

        let o = s
            .create_object(
                None,
                ObjAttrs::new()
                    .name("test")
                    .flags(ObjFlag::Write | ObjFlag::Read),
            )
            .unwrap();

        let attrs = s
            .object_get_attrs(o, ObjAttr::Flags | ObjAttr::Name)
            .unwrap();

        assert_eq!(attrs.name.unwrap(), "test");
        assert!(attrs.flags.unwrap().contains(ObjFlag::Write));

        s.commit().unwrap();
    }

    #[test]
    fn test_inheritance_chain() {
        let mut odb = ImDB::default();

        // Create objects and establish parent-child relationship
        let o1 = odb.create_object(Some(Objid(1)), ObjAttrs::new().name("o1")).unwrap();
        let o2 = odb
            .create_object(Some(Objid(2)), ObjAttrs::new().name("o2").parent(o1))
            .unwrap();
        let _o3 = odb
            .create_object(Some(Objid(3)), ObjAttrs::new().name("o3").parent(o2))
            .unwrap();
        let _o4 = odb
            .create_object(Some(Objid(4)), ObjAttrs::new().name("o4").parent(o2))
            .unwrap();
        let o5 = odb
            .create_object(Some(Objid(5)), ObjAttrs::new().name("o5").parent(o1))
            .unwrap();
        let o6 = odb
            .create_object(Some(Objid(6)), ObjAttrs::new().name("o6").parent(o5))
            .unwrap();

        // Test inheritance chain for o6
        let inheritance_chain = odb.get_object_inheritance_chain(o6);
        assert_eq!(inheritance_chain, vec![Objid(6), Objid(5), Objid(1)]);

        // Test inheritance chain for o2
        let inheritance_chain = odb.get_object_inheritance_chain(o2);
        assert_eq!(inheritance_chain, vec![Objid(2), Objid(1)]);

        // Test inheritance chain for o1
        let inheritance_chain = odb.get_object_inheritance_chain(o1);
        assert_eq!(inheritance_chain, vec![Objid(1)]);

        // Test inheritance chain for non-existent object
        let inheritance_chain = odb.get_object_inheritance_chain(Objid(7));
        assert_eq!(inheritance_chain, vec![]);

        // Test object_children for o1
        let mut children = odb.object_children(o1).unwrap();
        assert_eq!(children.sort(), vec![Objid(2), Objid(5)].sort());

        // Test object_children for o2
        let mut children = odb.object_children(o2).unwrap();
        assert_eq!(children.sort(), vec![Objid(3), Objid(4)].sort());

        // Test object_children for non-existent object
        let children = odb.object_children(Objid(7));
        assert!(children.is_err());
    }

    #[test]
    fn test_propdefs() {
        let mut odb = ImDB::default();

        // Add some property definitions.
        let pid1 = odb
            .add_propdef(Objid(1), "color", Objid(1), enum_set!(PropFlag::Read), None)
            .unwrap();
        let pid2 = odb
            .add_propdef(
                Objid(1),
                "size",
                Objid(2),
                PropFlag::Read | PropFlag::Write,
                Some(Var::Int(42)),
            )
            .unwrap();

        // Get a property definition by its name.
        let def1 = odb.get_propdef(Objid(1), "color").unwrap();
        assert_eq!(def1.pid, pid1);
        assert_eq!(def1.definer, Objid(1));
        assert_eq!(def1.pname, "color");

        // Rename a property.
        odb.rename_propdef(Objid(1), "color", "shade").unwrap();
        let def2 = odb.get_propdef(Objid(1), "shade").unwrap();
        assert_eq!(def2.pid, pid1);
        assert_eq!(def2.definer, Objid(1));
        assert_eq!(def2.pname, "shade");

        // Get all property definitions on an object.
        let defs = odb.get_propdefs(Objid(1)).unwrap();
        assert_eq!(defs.len(), 2);
        assert!(defs.contains(&def2));
        assert!(defs.contains(&Propdef {
            pid: pid2,
            definer: Objid(1),
            pname: "size".to_owned(),
        }));

        // Delete a property definition.
        odb.delete_propdef(Objid(1), "size").unwrap();
        let defs = odb.get_propdefs(Objid(1)).unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0], def2);

        // Count the number of property definitions on an object.
        let count = odb.count_propdefs(Objid(1)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn property_inheritance() {
        let mut s = ImDB::default();

        let parent = s.create_object(None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let pid = s
            .add_propdef(
                parent,
                "test",
                parent,
                PropFlag::Chown | PropFlag::Read,
                Some(Var::Str(String::from("testing"))),
            )
            .unwrap();

        let pds = s.get_propdefs(parent).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, parent);
        assert_eq!(pds[0].pid, pid, "test");

        // Verify initially that we get the value all the way from root.
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(parent));

        // Set it on the intermediate child...
        s.set_property(
            pid,
            child1,
            Var::Str(String::from("testing")),
            parent,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child1));

        // Finally set it on the last child...
        s.set_property(
            pid,
            child2,
            Var::Str(String::from("testing")),
            parent,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        // And then verify we get it from there...
        let v = s
            .get_property(child2, pid, PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.location, Some(child2));

        // Finally, use the name to look it up instead of the pid
        let v = s
            .find_property(child2, "test", PropAttr::Value | PropAttr::Location)
            .unwrap()
            .unwrap();
        assert_eq!(v.attrs.location, Some(child2));
        // And verify we don't get it from other root or from its child
        let v = s
            .get_property(other_root, pid, PropAttr::Value | PropAttr::Location)
            .unwrap();
        assert!(v.is_none());

        s.commit().unwrap();
    }

    #[test]
    fn verb_inheritance() {
        let mut s = ImDB::default();

        let parent = s.create_object(None, &ObjAttrs::new()).unwrap();
        let child1 = s
            .create_object(None, ObjAttrs::new().parent(parent))
            .unwrap();
        let child2 = s
            .create_object(None, ObjAttrs::new().parent(child1))
            .unwrap();

        let other_root = s.create_object(None, &ObjAttrs::new()).unwrap();
        let _other_root_child = s
            .create_object(None, ObjAttrs::new().parent(other_root))
            .unwrap();

        let thisnonethis = VerbArgsSpec {
            dobj: ArgSpec::This,
            prep: PrepSpec::None,
            iobj: ArgSpec::This,
        };
        let _vinfo = s
            .add_verb(
                parent,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        let verbs = s
            .get_verbs(
                parent,
                VerbAttr::Definer | VerbAttr::Owner | VerbAttr::Flags | VerbAttr::ArgsSpec,
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
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), parent);

        // Set it on the intermediate child...
        let _vinfo = s
            .add_verb(
                child1,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child1);

        // Finally set it on the last child...
        let _vinfo = s
            .add_verb(
                child2,
                vec!["look_down", "look_up"],
                parent,
                VerbFlag::Exec | VerbFlag::Read,
                thisnonethis,
                Program(bytes::Bytes::new()),
            )
            .unwrap();

        // And then verify we get it from there...
        let v = s
            .find_callable_verb(
                child2,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_some());
        assert_eq!(v.unwrap().attrs.definer.unwrap(), child2);

        // And verify we don't get it from other root or from its child
        let v = s
            .find_callable_verb(
                other_root,
                "look_up",
                VerbAttr::Definer | VerbAttr::Flags | VerbAttr::ArgsSpec,
            )
            .unwrap();
        assert!(v.is_none());

        s.commit().unwrap();
    }
}