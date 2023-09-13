use moor_value::model::defset::HasUuid;
use moor_value::model::objects::{ObjAttrs, ObjFlag};
use moor_value::model::objset::ObjSet;
use moor_value::model::propdef::{PropDef, PropDefs};
use moor_value::model::props::PropFlag;
use moor_value::model::r#match::VerbArgsSpec;
use moor_value::model::verbdef::{VerbDef, VerbDefs};
use moor_value::model::verbs::{BinaryType, VerbFlag};
use moor_value::model::WorldStateError;
use moor_value::model::WorldStateError::{ObjectNotFound, PropertyNotFound, VerbNotFound};
use moor_value::util::bitenum::BitEnum;
use moor_value::var::objid::Objid;
use moor_value::var::Var;
use moor_value::NOTHING;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(crate) struct TransientStore {
    max_object: usize,
    verbdefs: HashMap<Objid, VerbDefs>,
    verb_programs: HashMap<(Objid, Uuid), Vec<u8>>,
    objects: HashMap<Objid, Object>,
    propdefs: HashMap<Objid, PropDefs>,
    properties: HashMap<(Objid, Uuid), Var>,
}

struct Object {
    name: String,
    flags: BitEnum<ObjFlag>,
    owner: Objid,
    location: Objid,
    contents: ObjSet,
    parent: Objid,
    children: ObjSet,
}

impl TransientStore {
    pub fn new() -> Self {
        Self {
            max_object: 0,
            verbdefs: Default::default(),
            verb_programs: Default::default(),
            objects: Default::default(),
            propdefs: Default::default(),
            properties: Default::default(),
        }
    }
}
impl TransientStore {
    pub fn object_valid(&self, o: Objid) -> Result<bool, WorldStateError> {
        Ok(self.objects.contains_key(&o))
    }
    pub fn get_max_object(&self) -> Result<Objid, WorldStateError> {
        Ok(Objid(self.max_object as i64))
    }
    pub fn create_object(
        &mut self,
        id: Option<Objid>,
        attrs: ObjAttrs,
    ) -> Result<Objid, WorldStateError> {
        let obj = Object {
            name: attrs.name.unwrap_or("".to_string()),
            flags: attrs.flags.unwrap_or(BitEnum::new()),
            owner: attrs.owner.unwrap_or(NOTHING),
            location: NOTHING,
            contents: ObjSet::new(),
            parent: NOTHING,
            children: ObjSet::new(),
        };
        let id = match id {
            None => {
                let o = Objid(self.max_object as i64);
                o
            }
            Some(id) => {
                if self.objects.contains_key(&id) {
                    return Err(WorldStateError::ObjectAlreadyExists(id));
                }
                id
            }
        };

        self.objects.insert(id, obj);

        if id.0 >= self.max_object as i64 {
            self.max_object = id.0 as usize + 1;
        }

        if let Some(parent) = attrs.parent {
            self.set_object_parent(id, parent)?;
        }

        if let Some(location) = attrs.location {
            self.set_object_location(id, location)?;
        }

        Ok(id)
    }
    pub fn recycle_object(&mut self, obj: Objid) -> Result<(), WorldStateError> {
        // First go through and move all objects that are in this object's contents to the
        // to #-1.  It's up to the caller here to execute :exitfunc on all of them.
        let contents = self.get_object_contents(obj)?;
        for c in contents.iter() {
            self.set_object_location(c, NOTHING)?;
        }

        // Now reparent all our immediate children to our parent.
        // This should properly move all properties all the way down the chain.
        let parent = self.get_object_parent(obj)?;
        let children = self.get_object_children(obj)?;
        for c in children.iter() {
            self.set_object_parent(c, parent)?;
        }

        // Now it's safe to destroy the object.
        self.objects.remove(&obj);

        Ok(())
    }
    pub fn get_object_location(&self, o: Objid) -> Result<Objid, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.location)
    }

    pub fn get_object_contents(&self, o: Objid) -> Result<ObjSet, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.contents.clone())
    }

    pub fn set_object_location(&mut self, o: Objid, loc: Objid) -> Result<(), WorldStateError> {
        if !self.objects.contains_key(&o) {
            return Err(ObjectNotFound(o));
        }
        let mut oid = loc;
        loop {
            if oid == NOTHING {
                break;
            }
            if oid == o {
                return Err(WorldStateError::RecursiveMove(o, loc));
            }
            oid = self.get_object_location(oid).unwrap_or(NOTHING);
        }

        let obj = self.objects.get_mut(&o).unwrap();
        let old_location = obj.location;
        obj.location = loc;
        if old_location != NOTHING {
            let old_loc_obj = self.objects.get_mut(&old_location).unwrap();
            let updated_old_contents = old_loc_obj.contents.with_removed(o);
            old_loc_obj.contents = updated_old_contents;
        }
        if loc != NOTHING {
            let new_loc_obj = self.objects.get_mut(&loc).unwrap();
            let updated_new_contents = new_loc_obj.contents.with_inserted(o);
            new_loc_obj.contents = updated_new_contents;
        }
        Ok(())
    }

    pub fn get_flags_of(&self, o: Objid) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.flags)
    }

    pub fn set_flags_of(&mut self, o: Objid, f: BitEnum<ObjFlag>) -> Result<(), WorldStateError> {
        let Some(obj) = self.objects.get_mut(&o) else {
            return Err(ObjectNotFound(o));
        };
        obj.flags = f;
        Ok(())
    }

    pub fn get_object_name(&self, o: Objid) -> Result<String, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.name.clone())
    }

    pub fn set_object_name(&mut self, o: Objid, n: String) -> Result<(), WorldStateError> {
        let Some(obj) = self.objects.get_mut(&o) else {
            return Err(ObjectNotFound(o));
        };
        obj.name = n;
        Ok(())
    }

    pub fn get_object_parent(&self, o: Objid) -> Result<Objid, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.parent)
    }

    pub fn set_object_parent(
        &mut self,
        o: Objid,
        new_parent: Objid,
    ) -> Result<(), WorldStateError> {
        if !self.objects.contains_key(&o) {
            return Err(ObjectNotFound(o));
        }

        let (_shared_ancestor, new_ancestors, old_ancestors) =
            self.closest_common_ancestor_with_ancestors(new_parent, o);

        // Remove properties defined by old ancestors
        let old_props = self.get_propdefs(o)?;
        let mut delort_props = vec![];
        for p in old_props.iter() {
            if old_ancestors.contains(&p.definer()) {
                delort_props.push(p.uuid());
                self.properties.remove(&(o, p.uuid()));
            }
        }
        let new_props = old_props.with_all_removed(&delort_props);
        self.propdefs.insert(o, new_props);

        // And now do the same for all my soon-to-be-orphaned children.
        let descendants = self.descendants(o);
        let mut descendant_props = HashMap::new();
        for c in descendants.iter() {
            let mut inherited_props = vec![];
            // Remove the set values.
            let old_props = self.get_propdefs(c)?;
            for p in old_props.iter() {
                if old_ancestors.contains(&p.definer()) {
                    inherited_props.push(p.uuid());
                    self.properties.remove(&(c, p.uuid()));
                }
            }
            // And update the (new) propdefs to not include them
            let new_props = old_props.with_all_removed(&inherited_props);

            // We're not actually going to *set* these yet because we are going to add, later.
            descendant_props.insert(c, new_props);
        }

        let obj = self.objects.get_mut(&o).unwrap();
        let old_parent = obj.parent;
        obj.parent = new_parent;
        if old_parent != NOTHING {
            let old_parent_obj = self.objects.get_mut(&old_parent).unwrap();
            let updated_old_children = old_parent_obj.children.with_removed(o);
            old_parent_obj.children = updated_old_children;
        }
        if new_parent != NOTHING {
            let new_parent_obj = self.objects.get_mut(&new_parent).unwrap();
            let updated_new_children = new_parent_obj.children.with_inserted(o);
            new_parent_obj.children = updated_new_children;
        }

        // Now give all my new children the new properties that derive from ancestors they don't
        // already share...
        let mut new_props = vec![];
        for a in new_ancestors {
            let props = self.get_propdefs(a)?;
            for p in props.iter() {
                if p.definer() == a {
                    new_props.push(p.clone())
                }
            }
        }
        let descendants = self.descendants(o);
        for c in descendants.iter().chain(std::iter::once(o)) {
            // Check if we have a cached/modified copy from above in descendant_props
            let c_props = match descendant_props.remove(&c) {
                None => self.get_propdefs(c)?,
                Some(props) => props,
            };
            let c_props = c_props.with_all_added(&new_props);
            self.propdefs.insert(c, c_props);
        }
        Ok(())
    }

    pub fn get_object_children(&self, o: Objid) -> Result<ObjSet, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.children.clone())
    }

    pub fn get_object_owner(&self, o: Objid) -> Result<Objid, WorldStateError> {
        let Some(obj) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        Ok(obj.owner)
    }

    pub fn set_object_owner(&mut self, o: Objid, no: Objid) -> Result<(), WorldStateError> {
        let Some(obj) = self.objects.get_mut(&o) else {
            return Err(ObjectNotFound(o));
        };
        obj.owner = no;
        Ok(())
    }

    pub fn get_verbdefs(&self, o: Objid) -> Result<VerbDefs, WorldStateError> {
        let Some(verbdefs) = self.verbdefs.get(&o) else {
            return Ok(VerbDefs::empty());
        };
        Ok(verbdefs.clone())
    }

    pub fn get_verb_by_name(&self, o: Objid, n: String) -> Result<VerbDef, WorldStateError> {
        let Some(verbdefs) = self.verbdefs.get(&o) else {
            return Err(VerbNotFound(o, n));
        };
        // TODO: verify that all uses of this are actually needing this "just grab the first match"
        let verbdef = verbdefs.find_first_named(n.as_str());
        let Some(verbdef) = verbdef else {
            return Err(VerbNotFound(o, n));
        };
        Ok(verbdef.clone())
    }

    pub fn get_verb_by_index(&self, o: Objid, idx: usize) -> Result<VerbDef, WorldStateError> {
        let Some(verbdefs) = self.verbdefs.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        let verbdef = verbdefs.iter().nth(idx);
        let Some(verbdef) = verbdef else {
            return Err(VerbNotFound(o, idx.to_string()));
        };
        Ok(verbdef.clone())
    }

    pub fn get_binary(&self, o: Objid, uuid: Uuid) -> Result<Vec<u8>, WorldStateError> {
        let Some(binary) = self.verb_programs.get(&(o, uuid)) else {
            return Err(VerbNotFound(o, uuid.to_string()));
        };
        Ok(binary.clone())
    }

    pub fn resolve_verb(
        &self,
        location: Objid,
        name: String,
        argspec: Option<VerbArgsSpec>,
    ) -> Result<VerbDef, WorldStateError> {
        let Some(_) = self.objects.get(&location) else {
            return Err(ObjectNotFound(location));
        };
        let mut search_o = location;
        loop {
            if let Some(verbs) = self.verbdefs.get(&search_o) {
                // Seek through the set of matches, looking for one that matches the argspec, if
                // we care about that. If not, just return the first one.
                let name_matches = verbs.find_named(name.as_str());
                for verb in name_matches {
                    match argspec {
                        Some(argspec) => {
                            if verb.args().matches(&argspec) {
                                return Ok(verb.clone());
                            }
                        }
                        None => {
                            return Ok(verb.clone());
                        }
                    }
                }
            };
            // Otherwise, find our parent.  If it's, then set o to it and continue unless we've
            // hit the end of the chain.
            let Some(o) = self.objects.get(&search_o) else {
                break;
            };
            let parent = o.parent;
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        Err(VerbNotFound(location, name))
    }

    pub fn update_verbdef(
        &mut self,
        obj: Objid,
        uuid: Uuid,
        owner: Option<Objid>,
        names: Option<Vec<String>>,
        flags: Option<BitEnum<VerbFlag>>,
        binary_type: Option<BinaryType>,
        args: Option<VerbArgsSpec>,
    ) -> Result<(), WorldStateError> {
        let Some(verbdefs) = self.verbdefs.get_mut(&obj) else {
            return Err(ObjectNotFound(obj));
        };
        let Some(new_verbs) = verbdefs.with_updated(uuid, |ov| {
            let names = match &names {
                None => ov.names(),
                Some(new_names) => new_names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            };
            VerbDef::new(
                ov.uuid(),
                ov.location(),
                owner.unwrap_or(ov.owner()),
                &names,
                flags.unwrap_or(ov.flags()),
                binary_type.unwrap_or(ov.binary_type()),
                args.unwrap_or(ov.args()),
            )
        }) else {
            let v_uuid_str = uuid.to_string();
            return Err(VerbNotFound(obj, v_uuid_str));
        };
        self.verbdefs.insert(obj, new_verbs);

        Ok(())
    }

    pub fn set_verb_binary(
        &mut self,
        obj: Objid,
        uuid: Uuid,
        binary: Vec<u8>,
    ) -> Result<(), WorldStateError> {
        self.verb_programs.insert((obj, uuid), binary);
        Ok(())
    }

    pub fn add_object_verb(
        &mut self,
        location: Objid,
        owner: Objid,
        names: Vec<String>,
        binary: Vec<u8>,
        binary_type: BinaryType,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
    ) -> Result<(), WorldStateError> {
        let verbdefs = self
            .verbdefs
            .entry(location)
            .or_insert_with(VerbDefs::empty);
        let uuid = Uuid::new_v4();
        let new_verbs = verbdefs.with_added(VerbDef::new(
            uuid,
            location,
            owner,
            &names.iter().map(|n| n.as_str()).collect::<Vec<&str>>(),
            flags,
            binary_type,
            args,
        ));
        self.verbdefs.insert(location, new_verbs);
        self.verb_programs.insert((location, uuid), binary);
        Ok(())
    }

    pub fn delete_verb(&mut self, location: Objid, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbdefs = self
            .verbdefs
            .entry(location)
            .or_insert_with(VerbDefs::empty);
        let new_verbs = verbdefs.with_removed(uuid).unwrap();
        self.verbdefs.insert(location, new_verbs);
        self.verb_programs.remove(&(location, uuid));
        Ok(())
    }

    pub fn get_propdefs(&self, o: Objid) -> Result<PropDefs, WorldStateError> {
        let Some(propdefs) = self.propdefs.get(&o) else {
            return Ok(PropDefs::empty());
        };
        Ok(propdefs.clone())
    }

    pub fn retrieve_property(&self, o: Objid, u: Uuid) -> Result<Var, WorldStateError> {
        let Some(prop_v) = self.properties.get(&(o, u)) else {
            return Err(PropertyNotFound(o, u.to_string()));
        };
        Ok(prop_v.clone())
    }

    pub fn set_property(&mut self, o: Objid, u: Uuid, v: Var) -> Result<(), WorldStateError> {
        self.properties.insert((o, u), v);
        Ok(())
    }

    pub fn define_property(
        &mut self,
        definer: Objid,
        location: Objid,
        name: String,
        owner: Objid,
        perms: BitEnum<PropFlag>,
        value: Option<Var>,
    ) -> Result<Uuid, WorldStateError> {
        let descendants = self.descendants(location);
        let locations = ObjSet::from(&[location]).with_concatenated(descendants);
        let uuid = Uuid::new_v4();
        for location in locations.iter() {
            let new_propdef = PropDef::new(uuid, definer, location, name.as_str(), perms, owner);

            if let Some(propdefs) = self.propdefs.get_mut(&location) {
                // Verify we don't already have a property with this name. If we do, return an error.
                if propdefs.find_first_named(name.as_str()).is_some() {
                    return Err(WorldStateError::DuplicatePropertyDefinition(location, name));
                }

                *propdefs = propdefs.with_added(new_propdef);
            } else {
                self.propdefs
                    .insert(location, PropDefs::from_items(&[new_propdef]));
            }
        }

        if let Some(value) = value {
            self.properties.insert((definer, uuid), value);
        }

        Ok(uuid)
    }

    pub fn set_property_info(
        &mut self,
        obj: Objid,
        uuid: Uuid,
        new_owner: Option<Objid>,
        new_flags: Option<BitEnum<PropFlag>>,
        new_name: Option<String>,
    ) -> Result<(), WorldStateError> {
        let propdefs = self.propdefs.entry(obj).or_insert_with(PropDefs::empty);
        let new_propdefs = propdefs
            .with_updated(uuid, |op| {
                PropDef::new(
                    op.uuid(),
                    op.location(),
                    new_owner.unwrap_or(op.owner()),
                    new_name.clone().unwrap_or(op.name().to_string()).as_str(),
                    new_flags.unwrap_or(op.flags()),
                    op.owner(),
                )
            })
            .unwrap();
        self.propdefs.insert(obj, new_propdefs);
        Ok(())
    }

    pub fn clear_property(&mut self, o: Objid, u: Uuid) -> Result<(), WorldStateError> {
        self.properties.remove(&(o, u));
        Ok(())
    }

    pub fn delete_property(&mut self, o: Objid, u: Uuid) -> Result<(), WorldStateError> {
        let propdefs = self.propdefs.entry(o).or_insert_with(PropDefs::empty);
        let new_propdefs = propdefs.with_removed(u).unwrap();
        self.propdefs.insert(o, new_propdefs);
        self.properties.remove(&(o, u));
        Ok(())
    }

    pub fn resolve_property(&self, o: Objid, n: String) -> Result<(PropDef, Var), WorldStateError> {
        let Some(_) = self.objects.get(&o) else {
            return Err(ObjectNotFound(o));
        };
        // First we get the propdef locally. If we don't have this, the property don't exist.
        let Some(propdefs) = self.propdefs.get(&o) else {
            return Err(PropertyNotFound(o, n));
        };
        let Some(propdef) = propdefs.find_first_named(n.as_str()) else {
            return Err(PropertyNotFound(o, n));
        };

        // Now walk up the tree searching for the first non-clear value.
        let mut search_o = o;
        loop {
            if let Some(v) = self.properties.get(&(search_o, propdef.uuid())) {
                return Ok((propdef.clone(), v.clone()));
            }

            // Value was clear, walk up to the parent, if there is one.
            let Some(o) = self.objects.get(&search_o) else {
                break;
            };
            let parent = o.parent;
            if parent == NOTHING {
                break;
            }
            search_o = parent;
        }
        Err(PropertyNotFound(o, n))
    }
}

impl TransientStore {
    fn descendants(&self, obj: Objid) -> ObjSet {
        let mut search_queue = vec![obj];

        let all_children = std::iter::from_fn(move || {
            while let Some(search_obj) = search_queue.pop() {
                let Some(o) = self.objects.get(&search_obj) else {
                    continue;
                };
                let new_children = o.children.clone();
                search_queue.extend(new_children.iter());
                // Extend the iterator with new children
                return Some(new_children.iter());
            }
            None
        })
        .flatten();

        ObjSet::from_oid_iter(all_children)
    }
    fn closest_common_ancestor_with_ancestors(
        &self,
        a: Objid,
        b: Objid,
    ) -> (Option<Objid>, HashSet<Objid>, HashSet<Objid>) {
        let mut ancestors_a = HashSet::new();
        let mut search_a = a;

        let mut ancestors_b = HashSet::new();
        let mut search_b = b;

        loop {
            if search_a == NOTHING && search_b == NOTHING {
                return (None, ancestors_a, ancestors_b); // No common ancestor found
            }

            if ancestors_b.contains(&search_a) {
                return (Some(search_a), ancestors_a, ancestors_b); // Common ancestor found
            }

            if ancestors_a.contains(&search_b) {
                return (Some(search_b), ancestors_a, ancestors_b); // Common ancestor found
            }

            if search_a != NOTHING {
                ancestors_a.insert(search_a);
                let parent = self.objects.get(&search_a).unwrap().parent;
                search_a = parent;
            }

            if search_b != NOTHING {
                ancestors_b.insert(search_b);
                let parent = self.objects.get(&search_b).unwrap().parent;
                search_b = parent;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db::inmemtransient::transient_store::TransientStore;
    use moor_value::model::defset::HasUuid;
    use moor_value::model::objects::ObjAttrs;
    use moor_value::model::objset::ObjSet;
    use moor_value::model::r#match::VerbArgsSpec;
    use moor_value::model::verbs::BinaryType;
    use moor_value::model::WorldStateError;
    use moor_value::util::bitenum::BitEnum;
    use moor_value::var::objid::Objid;
    use moor_value::var::v_str;
    use moor_value::NOTHING;

    #[test]
    fn test_create_object() {
        let mut db = TransientStore::new();
        let oid = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        assert_eq!(oid, Objid(0));
        assert!(db.object_valid(oid).unwrap());
        assert_eq!(db.get_object_owner(oid).unwrap(), NOTHING);
        assert_eq!(db.get_object_parent(oid).unwrap(), NOTHING);
        assert_eq!(db.get_object_location(oid).unwrap(), NOTHING);
        assert_eq!(db.get_object_name(oid).unwrap(), "test");
    }

    #[test]
    fn test_parent_children() {
        let mut db = TransientStore::new();

        // Single parent/child relationship.
        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(db.get_object_parent(b).unwrap(), a);
        assert_eq!(db.get_object_children(a).unwrap(), ObjSet::from(&[b]));

        assert_eq!(db.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(db.get_object_children(b).unwrap(), ObjSet::new());

        // Add a second child
        let c = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(db.get_object_parent(c).unwrap(), a);
        assert_eq!(db.get_object_children(a).unwrap(), ObjSet::from(&[b, c]));

        assert_eq!(db.get_object_parent(a).unwrap(), NOTHING);
        assert_eq!(db.get_object_children(b).unwrap(), ObjSet::new());

        // Create new obj and reparent one child
        let d = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.set_object_parent(b, d).unwrap();
        assert_eq!(db.get_object_parent(b).unwrap(), d);
        assert_eq!(db.get_object_children(a).unwrap(), ObjSet::from(&[c]));
        assert_eq!(db.get_object_children(d).unwrap(), ObjSet::from(&[b]));
    }

    #[test]
    fn test_descendants() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let c = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let d = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(c),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(db.descendants(a), ObjSet::from(&[b, c, d]));
        assert_eq!(db.descendants(b), ObjSet::new());
        assert_eq!(db.descendants(c), ObjSet::from(&[d]));

        // Now reparent d to b
        db.set_object_parent(d, b).unwrap();
        assert_eq!(db.get_object_children(a).unwrap(), ObjSet::from(&[b, c]));
        assert_eq!(db.get_object_children(b).unwrap(), ObjSet::from(&[d]));
        assert_eq!(db.get_object_children(c).unwrap(), ObjSet::new());
        assert_eq!(db.descendants(a), ObjSet::from(&[b, c, d]));
        assert_eq!(db.descendants(b), ObjSet::from(&[d]));
        assert_eq!(db.descendants(c), ObjSet::new());
    }

    #[test]
    fn test_location_contents() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(NOTHING),
                    location: Some(a),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        assert_eq!(db.get_object_location(b).unwrap(), a);
        assert_eq!(db.get_object_contents(a).unwrap(), ObjSet::from(&[b]));

        assert_eq!(db.get_object_location(a).unwrap(), NOTHING);
        assert_eq!(db.get_object_contents(b).unwrap(), ObjSet::new());

        let c = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.set_object_location(b, c).unwrap();
        assert_eq!(db.get_object_location(b).unwrap(), c);
        assert_eq!(db.get_object_contents(a).unwrap(), ObjSet::new());
        assert_eq!(db.get_object_contents(c).unwrap(), ObjSet::from(&[b]));

        let d = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test4".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();
        db.set_object_location(d, c).unwrap();
        assert_eq!(db.get_object_contents(c).unwrap(), ObjSet::from(&[b, d]));
        assert_eq!(db.get_object_location(d).unwrap(), c);

        db.set_object_location(a, c).unwrap();
        assert_eq!(db.get_object_contents(c).unwrap(), ObjSet::from(&[b, d, a]));
        assert_eq!(db.get_object_location(a).unwrap(), c);

        // Validate recursive move detection.
        match db.set_object_location(c, b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // Move b one level deeper, and then check recursive move detection again.
        db.set_object_location(b, d).unwrap();
        match db.set_object_location(c, b).err() {
            Some(WorldStateError::RecursiveMove(_, _)) => {}
            _ => {
                panic!("Expected recursive move error");
            }
        }

        // The other way around, d to c should be fine.
        db.set_object_location(d, c).unwrap();
    }

    #[test]
    fn test_simple_property() {
        let mut db = TransientStore::new();

        let oid = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.define_property(
            oid,
            oid,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test")),
        )
        .unwrap();
        let (prop, v) = db.resolve_property(oid, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test"));
    }

    #[test]
    fn test_transitive_property_resolution() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = db.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));

        // Verify we *don't* get this property for an unrelated, unhinged object by reparenting b
        // to new parent c.  This should remove the defs for a's properties from b.
        let c = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test3".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.set_object_parent(b, c).unwrap();

        let result = db.resolve_property(b, "test".into());
        assert_eq!(
            result.err().unwrap(),
            WorldStateError::PropertyNotFound(b, "test".into())
        );
    }

    #[test]
    fn test_transitive_property_resolution_clear_property() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let b = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test2".into()),
                    parent: Some(a),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.define_property(
            a,
            a,
            "test".into(),
            NOTHING,
            BitEnum::new(),
            Some(v_str("test_value")),
        )
        .unwrap();
        let (prop, v) = db.resolve_property(b, "test".into()).unwrap();
        assert_eq!(prop.name(), "test");
        assert_eq!(v, v_str("test_value"));

        // Define the property again, but on the object 'b',
        // This should raise an error because the child already *has* this property.
        // MOO will not let this happen. The right way to handle overloading is to set the value
        // on the child.
        let result = db.define_property(a, b, "test".into(), NOTHING, BitEnum::new(), None);
        assert!(matches!(
            result,
            Err(WorldStateError::DuplicatePropertyDefinition(_, _))
        ));
    }

    #[test]
    fn test_verb_resolve() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        db.add_object_verb(
            a,
            a,
            vec!["test".into()],
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            db.resolve_verb(a, "test".into(), None).unwrap().names(),
            vec!["test"]
        );

        assert_eq!(
            db.resolve_verb(a, "test".into(), Some(VerbArgsSpec::this_none_this()))
                .unwrap()
                .names(),
            vec!["test"]
        );

        let v_uuid = db.resolve_verb(a, "test".into(), None).unwrap().uuid();
        assert_eq!(db.get_binary(a, v_uuid).unwrap(), vec![]);
    }

    #[test]
    fn test_verb_resolve_wildcard() {
        let mut db = TransientStore::new();

        let a = db
            .create_object(
                None,
                ObjAttrs {
                    owner: Some(NOTHING),
                    name: Some("test".into()),
                    parent: Some(NOTHING),
                    location: Some(NOTHING),
                    flags: Some(BitEnum::new()),
                },
            )
            .unwrap();

        let verb_names = vec!["dname*c", "iname*c"];
        db.add_object_verb(
            a,
            a,
            verb_names.iter().map(|s| s.to_string()).collect(),
            vec![],
            BinaryType::LambdaMoo18X,
            BitEnum::new(),
            VerbArgsSpec::this_none_this(),
        )
        .unwrap();

        assert_eq!(
            db.resolve_verb(a, "dname".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            db.resolve_verb(a, "dnamec".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            db.resolve_verb(a, "iname".into(), None).unwrap().names(),
            verb_names
        );

        assert_eq!(
            db.resolve_verb(a, "inamec".into(), None).unwrap().names(),
            verb_names
        );
    }
}
