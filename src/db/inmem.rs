use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
use crate::model::permissions::Permissions;
use crate::model::props::{
    Pid, PropAttr, PropAttrs, PropDefs, PropFlag, Propdef, Properties, PropertyInfo,
};
use crate::model::r#match::VerbArgsSpec;
use crate::model::var::Error::{E_INVARG, E_PERM};
use crate::model::var::{Objid, Var, NOTHING};
use crate::model::verbs::{Program, VerbAttr, VerbAttrs, VerbFlag, VerbInfo, Verbs, Vid};
use crate::model::ObjDB;
use anyhow::{anyhow, Error};
use enumset::EnumSet;
use std::collections::{HashMap, HashSet};

//
pub struct ImDB {
    max_obj: usize,
    objects: HashSet<Objid>,
    obj_attr_location: HashMap<Objid, Objid>,
    obj_attr_owner: HashMap<Objid, Objid>,
    obj_attr_parent: HashMap<Objid, Objid>,
    obj_attr_name: HashMap<Objid, String>,
    obj_attr_flags: HashMap<Objid, EnumSet<ObjFlag>>,

    // TOOD build custom datastructure to handle this more efficiently.
    obj_contents: HashMap<Objid, HashSet<Objid>>,
    obj_children: HashMap<Objid, HashSet<Objid>>,
}

impl ImDB {
    pub fn new() -> Self {
        Self {
            max_obj: 0usize,
            objects: Default::default(),
            obj_attr_location: Default::default(),
            obj_attr_owner: Default::default(),
            obj_attr_parent: Default::default(),
            obj_attr_name: Default::default(),
            obj_attr_flags: Default::default(),
            obj_contents: Default::default(),
            obj_children: Default::default(),
        }
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
                let oid = self.max_obj;
                self.max_obj += 1;
                Objid(oid as i64)
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
            false => return Err(anyhow!("invalid object")),
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

impl Properties for ImDB {
    fn find_property(
        &self,
        oid: Objid,
        name: &str,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropertyInfo>, Error> {
        todo!()
    }

    fn get_property(
        &self,
        oid: Objid,
        handle: Pid,
        attrs: EnumSet<PropAttr>,
    ) -> Result<Option<PropAttrs>, Error> {
        todo!()
    }

    fn set_property(
        &self,
        handle: Pid,
        location: Objid,
        value: Var,
        owner: Objid,
        flags: EnumSet<PropFlag>,
    ) -> Result<(), Error> {
        todo!()
    }
}

impl PropDefs for ImDB {
    fn get_propdef(&mut self, definer: Objid, pname: &str) -> Result<Propdef, Error> {
        todo!()
    }

    fn add_propdef(
        &mut self,
        definer: Objid,
        name: &str,
        owner: Objid,
        flags: EnumSet<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<Pid, Error> {
        todo!()
    }

    fn rename_propdef(&mut self, definer: Objid, old: &str, new: &str) -> Result<(), Error> {
        todo!()
    }

    fn delete_propdef(&mut self, definer: Objid, pname: &str) -> Result<(), Error> {
        todo!()
    }

    fn count_propdefs(&mut self, definer: Objid) -> Result<usize, Error> {
        todo!()
    }

    fn get_propdefs(&mut self, definer: Objid) -> Result<Vec<Propdef>, Error> {
        todo!()
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
        todo!()
    }

    fn get_verbs(&self, oid: Objid, attrs: EnumSet<VerbAttr>) -> Result<Vec<VerbInfo>, Error> {
        todo!()
    }

    fn get_verb(&self, vid: Vid, attrs: EnumSet<VerbAttr>) -> Result<VerbInfo, Error> {
        todo!()
    }

    fn update_verb(&self, vid: Vid, attrs: VerbAttrs) -> Result<(), Error> {
        todo!()
    }

    fn find_command_verb(
        &self,
        oid: Objid,
        verb: &str,
        arg_spec: VerbArgsSpec,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }

    fn find_callable_verb(
        &self,
        oid: Objid,
        verb: &str,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }

    fn find_indexed_verb(
        &self,
        oid: Objid,
        index: usize,
        attrs: EnumSet<VerbAttr>,
    ) -> Result<Option<VerbInfo>, Error> {
        todo!()
    }
}

impl Permissions for ImDB {
    fn property_allows(
        &self,
        check_flags: EnumSet<PropFlag>,
        player: Objid,
        player_flags: EnumSet<ObjFlag>,
        prop_flags: EnumSet<PropFlag>,
        prop_owner: Objid,
    ) -> bool {
        todo!()
    }
}

impl ObjDB for ImDB {
    fn initialize(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn commit(self) -> Result<(), Error> {
        Ok(())
    }

    fn rollback(self) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::inmem::ImDB;
    use crate::model::objects::{ObjAttr, ObjAttrs, ObjFlag, Objects};
    use crate::model::props::{PropAttr, PropDefs, PropFlag, Properties};
    use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
    use crate::model::var::Var;
    use crate::model::verbs::{Program, VerbAttr, VerbFlag, Verbs};
    use crate::model::ObjDB;

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
    fobject_check_children_contents() {
        let mut s = ImDB::default();

        let o1 = s.create_object(None, ObjAttrs::new().name("test")).unwrap();
        let o2 = s
            .create_object(None, ObjAttrs::new().name("test2").location(o1).parent(o1))
            .unwrap();
        let o3 = s
            .create_object(None, ObjAttrs::new().name("test3").location(o1).parent(o1))
            .unwrap();

        let children = s.object_children(o1).unwrap();
        assert_eq!(children, vec![o2, o3]);

        let contents = s.object_contents(o1).unwrap();
        assert_eq!(contents, vec![o2, o3]);

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
    fn propdef_create_get_update_count_delete() {
        let mut s = ImDB::default();

        let o = s.create_object(None, &ObjAttrs::new()).unwrap();

        let pid = s
            .add_propdef(
                o,
                "test",
                o,
                PropFlag::Chown | PropFlag::Read,
                Some(Var::Str(String::from("testing"))),
            )
            .unwrap();

        let pds = s.get_propdefs(o).unwrap();
        assert_eq!(pds.len(), 1);
        assert_eq!(pds[0].definer, o);
        assert_eq!(pds[0].pname, "test");
        assert_eq!(pds[0].pid, pid);

        s.rename_propdef(o, "test", "test2").unwrap();

        s.set_property(
            pds[0].pid,
            o,
            Var::Str(String::from("testing")),
            o,
            PropFlag::Read | PropFlag::Write,
        )
        .unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 1);

        s.delete_propdef(o, "test2").unwrap();

        let c = s.count_propdefs(o).unwrap();
        assert_eq!(c, 0);
        s.commit().unwrap();
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
