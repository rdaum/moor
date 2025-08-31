// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use ahash::HashSet;
use lazy_static::lazy_static;
use uuid::Uuid;

use crate::moor_db::WorldStateTransaction;
use moor_common::model::Perms;
use moor_common::model::WorldState;
use moor_common::model::WorldStateError;
use moor_common::model::{ArgSpec, ObjectKind, PrepSpec, VerbArgsSpec};
use moor_common::model::{CommitResult, PropPerms, ValSet};
use moor_common::model::{HasUuid, ObjectRef};
use moor_common::model::{ObjAttrs, ObjFlag};
use moor_common::model::{ObjSet, WorldStatePerf};
use moor_common::model::{PropAttrs, PropFlag};
use moor_common::model::{PropDef, PropDefs};
use moor_common::model::{VerbAttrs, VerbFlag};
use moor_common::model::{VerbDef, VerbDefs};
use moor_common::util::{BitEnum, PerfTimerGuard};
use moor_var::NOTHING;
use moor_var::Variant;
use moor_var::program::ProgramType;
use moor_var::{Obj, v_bool_int};
use moor_var::{Symbol, v_list};
use moor_var::{Var, v_obj};

lazy_static! {
    static ref NAME_SYM: Symbol = Symbol::mk("name");
    static ref LOCATION_SYM: Symbol = Symbol::mk("location");
    static ref CONTENTS_SYM: Symbol = Symbol::mk("contents");
    static ref OWNER_SYM: Symbol = Symbol::mk("owner");
    static ref CHILDREN_SYM: Symbol = Symbol::mk("children");
    static ref PARENT_SYM: Symbol = Symbol::mk("parent");
    static ref PROGRAMMER_SYM: Symbol = Symbol::mk("programmer");
    static ref WIZARD_SYM: Symbol = Symbol::mk("wizard");
    static ref R_SYM: Symbol = Symbol::mk("r");
    static ref W_SYM: Symbol = Symbol::mk("w");
    static ref F_SYM: Symbol = Symbol::mk("f");
    static ref ALIASES_SYM: Symbol = Symbol::mk("aliases");
    static ref WORLD_STATE_PERF: WorldStatePerf = WorldStatePerf::new();
}

pub fn db_counters<'a>() -> &'a WorldStatePerf {
    &WORLD_STATE_PERF
}

pub struct DbWorldState {
    pub tx: WorldStateTransaction,
}

impl DbWorldState {
    pub(crate) fn get_tx(&self) -> &WorldStateTransaction {
        &self.tx
    }

    pub(crate) fn get_tx_mut(&mut self) -> &mut WorldStateTransaction {
        &mut self.tx
    }
    fn perms(&self, who: &Obj) -> Result<Perms, WorldStateError> {
        let flags = self.flags_of(who)?;
        Ok(Perms { who: *who, flags })
    }

    fn do_update_verb(
        &mut self,
        obj: &Obj,
        perms: &Obj,
        verbdef: &VerbDef,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let perms = self.perms(perms)?;
        perms.check_verb_allows(&verbdef.owner(), verbdef.flags(), VerbFlag::Write)?;

        // If the verb code is being altered, a programmer or wizard bit is required.
        if verb_attrs.program.is_some()
            && !perms.check_is_wizard()?
            && !perms.flags.contains(ObjFlag::Programmer)
        {
            return Err(WorldStateError::VerbPermissionDenied);
        }

        self.get_tx_mut()
            .update_verb(obj, verbdef.uuid(), verb_attrs)?;
        Ok(())
    }

    fn check_parent(&self, perms: &Obj, parent: &Obj, owner: &Obj) -> Result<(), WorldStateError> {
        let (parentflags, parentowner) = (self.flags_of(parent)?, self.owner_of(parent)?);
        let createorperms = self.perms(perms)?;
        if self.valid(parent)? {
            createorperms.check_object_allows(
                &parentowner,
                parentflags,
                BitEnum::new_with(ObjFlag::Fertile),
            )?;
        } else if parent.ne(&NOTHING) || (owner.ne(perms) && !createorperms.check_is_wizard()?) {
            return Err(WorldStateError::ObjectPermissionDenied);
        }
        Ok(())
    }

    fn check_chparent_property_conflict(
        &self,
        perms: &Obj,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError> {
        // If object or one of its descendants defines a property with the same name as one defined
        // either on new-parent or on one of its ancestors, then E_INVARG is raised.
        let obj_or_descendant_props = self
            .descendants_of(perms, obj, true)?
            .iter()
            .map(|descendant| self.get_tx().get_properties(&descendant))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten();
        let new_parent_or_ancestors_property_names: HashSet<_> = self
            .ancestors_of(perms, new_parent, true)?
            .iter()
            .map(|ancestor| self.get_tx().get_properties(&ancestor))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .map(|prop| prop.name())
            .collect();
        for obj_or_descendant_prop in obj_or_descendant_props {
            if new_parent_or_ancestors_property_names.contains(&obj_or_descendant_prop.name()) {
                return Err(WorldStateError::ChparentPropertyNameConflict(
                    *obj,
                    *new_parent,
                    obj_or_descendant_prop.name().to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl WorldState for DbWorldState {
    fn players(&self) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.players);
        self.get_tx().get_players()
    }

    fn owner_of(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.owner_of);
        self.get_tx().get_object_owner(obj)
    }

    fn controls(&self, who: &Obj, what: &Obj) -> Result<bool, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.controls);
        let flags = self.flags_of(who)?;
        if flags.contains(ObjFlag::Wizard) {
            return Ok(true);
        }
        if who == what {
            return Ok(true);
        }
        let owner = self.owner_of(what)?;
        if owner == *who {
            return Ok(true);
        }
        Ok(false)
    }

    fn flags_of(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.flags_of);
        self.get_tx().get_object_flags(obj)
    }

    fn set_flags_of(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.set_flags_of);
        // Owner or wizard only.
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;
        self.get_tx_mut().set_object_flags(obj, new_flags)
    }

    fn location_of(&self, _perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.location_of);
        // MOO permits location query even if the object is unreadable!
        self.get_tx().get_object_location(obj)
    }

    fn object_bytes(&self, perms: &Obj, obj: &Obj) -> Result<usize, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.object_bytes);
        self.perms(perms)?.check_wizard()?;
        self.get_tx().get_object_size_bytes(obj)
    }

    fn create_object(
        &mut self,
        perms: &Obj,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
        id_kind: ObjectKind,
    ) -> Result<Obj, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.create_object);
        let is_wizard = self.perms(perms)?.check_is_wizard()?;
        if !self.valid(parent)? && (!parent.is_nothing() || (owner != perms && !is_wizard)) {
            return Err(WorldStateError::ObjectPermissionDenied);
        }

        if !is_wizard && owner != perms {
            return Err(WorldStateError::ObjectPermissionDenied);
        }

        // Handle different ID kinds - validate specific IDs exist check
        match &id_kind {
            ObjectKind::Objid(obj_id) => {
                // If a specific ID is requested, check if it already exists
                if self.valid(obj_id)? {
                    return Err(WorldStateError::ObjectAlreadyExists(*obj_id));
                }
            }
            ObjectKind::NextObjid | ObjectKind::UuObjId => {
                // No validation needed for auto-generated IDs
            }
        }

        self.check_parent(perms, parent, owner)?;

        // TODO: ownership_quota support
        //    If the intended owner of the new object has a property named `ownership_quota' and the value of that property is an integer, then `create()' treats that value
        //    as a "quota".  If the quota is less than or equal to zero, then the quota is considered to be exhausted and `create()' raises `E_QUOTA' instead of creating an
        //    object.  Otherwise, the quota is decremented and stored back into the `ownership_quota' property as a part of the creation of the new object.
        let attrs = ObjAttrs::new(*owner, *parent, NOTHING, flags, "");
        self.get_tx_mut().create_object(id_kind, attrs)
    }

    fn recycle_object(&mut self, perms: &Obj, obj: &Obj) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.recycle_object);
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;

        self.get_tx_mut().recycle_object(obj)
    }

    fn max_object(&self, _perms: &Obj) -> Result<Obj, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.max_object);
        self.get_tx().get_max_object()
    }

    fn move_object(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_loc: &Obj,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.move_object);
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;

        self.get_tx_mut().set_object_location(obj, new_loc)
    }

    fn contents_of(&self, _perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.contents_of);
        // MOO does not do any perms checks on contents, pretty sure:
        // https://github.com/wrog/lambdamoo/blob/master/db_properties.c#L351
        self.get_tx().get_object_contents(obj)
    }

    fn verbs(&self, perms: &Obj, obj: &Obj) -> Result<VerbDefs, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.verbs);
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Read.into())?;

        self.get_tx().get_verbs(obj)
    }

    fn properties(&self, perms: &Obj, obj: &Obj) -> Result<PropDefs, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.properties);
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Read.into())?;

        let properties = self.get_tx().get_properties(obj)?;
        Ok(properties)
    }

    #[allow(clippy::obfuscated_if_else)]
    fn retrieve_property(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Var, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.retrieve_property);
        if *obj == NOTHING || !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }

        // Special properties like name, location, and contents get treated specially.
        if pname == *NAME_SYM {
            return self.name_of(perms, obj).map(Var::from);
        } else if pname == *LOCATION_SYM {
            return self.location_of(perms, obj).map(Var::from);
        } else if pname == *CONTENTS_SYM {
            let contents: Vec<_> = self.contents_of(perms, obj)?.iter().map(v_obj).collect();
            return Ok(v_list(&contents));
        } else if pname == *OWNER_SYM {
            return self.owner_of(obj).map(Var::from);
        } else if pname == *PROGRAMMER_SYM {
            let flags = self.flags_of(obj)?;
            return Ok(flags
                .contains(ObjFlag::Programmer)
                .then(|| v_bool_int(true))
                .unwrap_or(v_bool_int(false)));
        } else if pname == *WIZARD_SYM {
            let flags = self.flags_of(obj)?;
            return Ok(flags
                .contains(ObjFlag::Wizard)
                .then(|| v_bool_int(true))
                .unwrap_or(v_bool_int(false)));
        } else if pname == *R_SYM {
            let flags = self.flags_of(obj)?;
            return Ok(flags
                .contains(ObjFlag::Read)
                .then(|| v_bool_int(true))
                .unwrap_or(v_bool_int(false)));
        } else if pname == *W_SYM {
            let flags = self.flags_of(obj)?;
            return Ok(flags
                .contains(ObjFlag::Write)
                .then(|| v_bool_int(true))
                .unwrap_or(v_bool_int(false)));
        } else if pname == *F_SYM {
            let flags = self.flags_of(obj)?;
            return Ok(flags
                .contains(ObjFlag::Fertile)
                .then(|| v_bool_int(true))
                .unwrap_or(v_bool_int(false)));
        }

        let (_, value, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Read)?;
        Ok(value)
    }

    fn get_property_info(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(PropDef, PropPerms), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.get_property_info);
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Read)?;

        Ok((pdef.clone(), propperms))
    }

    fn set_property_info(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.set_property_info);
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Write)?;

        // TODO Also keep a close eye on 'clear' & perms:
        //  "raises `E_INVARG' if <owner> is not valid" & If <object> is the definer of the property
        //   <prop-name>, as opposed to an inheritor of the property, then `clear_property()' raises
        //   `E_INVARG'

        self.get_tx_mut().update_property_info(
            obj,
            pdef.uuid(),
            attrs.owner,
            attrs.flags,
            attrs.name,
        )?;
        Ok(())
    }

    fn update_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.update_property);
        // You have to use move/chparent for this kinda fun.
        if pname == *LOCATION_SYM
            || pname == *CONTENTS_SYM
            || pname == *PARENT_SYM
            || pname == *CHILDREN_SYM
        {
            return Err(WorldStateError::PropertyPermissionDenied);
        }

        if pname == *NAME_SYM
            || pname == *OWNER_SYM
            || pname == *R_SYM
            || pname == *W_SYM
            || pname == *F_SYM
        {
            let (mut flags, objowner) = (self.flags_of(obj)?, self.owner_of(obj)?);

            // User is either wizard or owner
            self.perms(perms)?
                .check_object_allows(&objowner, flags, ObjFlag::Write.into())?;
            if pname == *NAME_SYM {
                let Some(name) = value.as_string() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                self.get_tx_mut().set_object_name(obj, name.to_string())?;
                return Ok(());
            }

            if pname == *OWNER_SYM {
                let Some(owner) = value.as_object() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                self.get_tx_mut().set_object_owner(obj, &owner)?;
                return Ok(());
            }

            if pname == *R_SYM {
                let Some(v) = value.as_integer() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if v == 1 {
                    flags.set(ObjFlag::Read);
                } else {
                    flags.clear(ObjFlag::Read);
                }
                self.get_tx_mut().set_object_flags(obj, flags)?;
                return Ok(());
            }

            if pname == *W_SYM {
                let Some(v) = value.as_integer() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if v == 1 {
                    flags.set(ObjFlag::Write);
                } else {
                    flags.clear(ObjFlag::Write);
                }
                self.get_tx_mut().set_object_flags(obj, flags)?;
                return Ok(());
            }

            if pname == *F_SYM {
                let Some(v) = value.as_integer() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if v == 1 {
                    flags.set(ObjFlag::Fertile);
                } else {
                    flags.clear(ObjFlag::Fertile);
                }
                self.get_tx_mut().set_object_flags(obj, flags)?;
                return Ok(());
            }
        }

        if pname == *PROGRAMMER_SYM || pname == *WIZARD_SYM {
            // Caller *must* be a wizard for either of these.
            self.perms(perms)?.check_wizard()?;

            // Gott get and then set flags
            let mut flags = self.flags_of(obj)?;
            if pname == *PROGRAMMER_SYM {
                if value.is_true() {
                    flags.set(ObjFlag::Programmer);
                } else {
                    flags.clear(ObjFlag::Programmer);
                }
            } else if pname == *WIZARD_SYM {
                if value.is_true() {
                    flags.set(ObjFlag::Wizard);
                } else {
                    flags.clear(ObjFlag::Wizard);
                }
            }

            self.get_tx_mut().set_object_flags(obj, flags)?;
            return Ok(());
        }

        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Write)?;

        self.get_tx_mut()
            .set_property(obj, pdef.uuid(), value.clone())?;
        Ok(())
    }

    fn is_property_clear(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<bool, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.is_property_clear);
        let (_, _, propperms, clear) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Read)?;
        Ok(clear)
    }

    fn clear_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.clear_property);
        // This is just deleting the local *value* portion of the property.
        // First seek the property handle.
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Write)?;
        self.get_tx_mut().clear_property(obj, pdef.uuid())?;
        Ok(())
    }

    fn define_property(
        &mut self,
        perms: &Obj,
        definer: &Obj,
        location: &Obj,
        pname: Symbol,
        propowner: &Obj,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.define_property);

        // Check if trying to define a builtin property name
        if pname == *NAME_SYM
            || pname == *LOCATION_SYM
            || pname == *CONTENTS_SYM
            || pname == *OWNER_SYM
            || pname == *PROGRAMMER_SYM
            || pname == *WIZARD_SYM
            || pname == *R_SYM
            || pname == *W_SYM
            || pname == *F_SYM
            || pname == *PARENT_SYM
            || pname == *CHILDREN_SYM
            || pname == *ALIASES_SYM
        {
            return Err(WorldStateError::PropertyPermissionDenied);
        }

        // Perms needs to be wizard, or have write permission on object *and* the owner in prop_flags
        // must be the perms
        let (flags, objowner) = (self.flags_of(location)?, self.owner_of(location)?);
        self.perms(perms)?
            .check_object_allows(&objowner, flags, ObjFlag::Write.into())?;
        self.perms(perms)?.check_obj_owner_perms(propowner)?;

        self.get_tx_mut().define_property(
            definer,
            location,
            pname,
            propowner,
            prop_flags,
            initial_value,
        )?;
        Ok(())
    }

    fn delete_property(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.delete_property);
        let properties = self.get_tx().get_properties(obj)?;
        let pdef = properties
            .find_first_named(pname)
            .ok_or_else(|| WorldStateError::PropertyNotFound(*obj, pname.to_string()))?;
        let propperms = self
            .get_tx()
            .retrieve_property_permissions(obj, pdef.uuid())?;
        self.perms(perms)?
            .check_property_allows(&propperms, PropFlag::Write)?;

        self.get_tx_mut().delete_property(obj, pdef.uuid())
    }

    fn add_verb(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        names: Vec<Symbol>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.add_verb);
        let (objflags, obj_owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&obj_owner, objflags, ObjFlag::Write.into())?;

        self.get_tx_mut()
            .add_object_verb(obj, owner, &names, program, flags, args)?;
        Ok(())
    }

    fn remove_verb(&mut self, perms: &Obj, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.remove_verb);
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Write)?;

        self.get_tx_mut().delete_verb(obj, vh.uuid())?;
        Ok(())
    }

    fn update_verb(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        vname: Symbol,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.update_verb);
        let vh = self.get_tx().get_verb_by_name(obj, vname)?;
        self.do_update_verb(obj, perms, &vh, verb_attrs)
    }

    fn update_verb_at_index(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        vidx: usize,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.update_verb);
        let vh = self.get_tx().get_verb_by_index(obj, vidx)?;
        self.do_update_verb(obj, perms, &vh, verb_attrs)
    }

    fn update_verb_with_id(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.update_verb);
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.do_update_verb(obj, perms, &vh, verb_attrs)
    }

    fn get_verb(&self, perms: &Obj, obj: &Obj, vname: Symbol) -> Result<VerbDef, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.get_verb);
        if !self.get_tx().object_valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }

        let vh = self.get_tx().get_verb_by_name(obj, vname)?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;

        Ok(vh)
    }

    fn get_verb_at_index(
        &self,
        perms: &Obj,
        obj: &Obj,
        vidx: usize,
    ) -> Result<VerbDef, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.get_verb);
        let vh = self.get_tx().get_verb_by_index(obj, vidx)?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;
        Ok(vh)
    }

    fn retrieve_verb(
        &self,
        perms: &Obj,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.retrieve_verb);
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;
        let binary = self.get_tx().get_verb_program(&vh.location(), vh.uuid())?;
        Ok((binary, vh))
    }

    fn find_method_verb_on(
        &self,
        perms: &Obj,
        obj: &Obj,
        vname: Symbol,
    ) -> Result<(ProgramType, VerbDef), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.find_method_verb_on);
        let vh = self.get_tx().resolve_verb(
            obj,
            vname,
            None,
            Some(BitEnum::new_with(VerbFlag::Exec)),
        )?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;

        let binary = self.get_tx().get_verb_program(&vh.location(), vh.uuid())?;
        Ok((binary, vh))
    }

    fn find_command_verb_on(
        &self,
        perms: &Obj,
        obj: &Obj,
        command_verb: Symbol,
        dobj: &Obj,
        prep: PrepSpec,
        iobj: &Obj,
    ) -> Result<Option<(ProgramType, VerbDef)>, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.find_command_verb_on);
        if !self.valid(obj)? {
            return Ok(None);
        }

        // TODO: LambdaMOO does not enforce a readability check on the object itself before
        //  resolving verbs on it. So this code is commented out.  However I can see an argument
        //  for keeping this functionality as a toggle-able option.
        // let (objflags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        // self.perms(perms)?
        //     .check_object_allows(&owner, objflags, ObjFlag::Read.into())?;

        let spec_for_fn = |oid, pco: &Obj| -> ArgSpec {
            if pco == oid {
                ArgSpec::This
            } else if pco.is_nothing() {
                ArgSpec::None
            } else {
                ArgSpec::Any
            }
        };

        let dobj = spec_for_fn(obj, dobj);
        let iobj = spec_for_fn(obj, iobj);
        let argspec = VerbArgsSpec { dobj, prep, iobj };

        let vh = self
            .get_tx()
            .resolve_verb(obj, command_verb, Some(argspec), None);
        let vh = match vh {
            Ok(vh) => vh,
            Err(WorldStateError::VerbNotFound(_, _)) => {
                return Ok(None);
            }
            Err(e) => {
                return Err(e);
            }
        };

        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;

        let program = self.get_tx().get_verb_program(&vh.location(), vh.uuid())?;
        Ok(Some((program, vh)))
    }

    fn parent_of(&self, _perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.parent_of);
        self.get_tx().get_object_parent(obj)
    }

    fn change_parent(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.change_parent);
        {
            let mut curr = *new_parent;
            while !curr.is_nothing() {
                if &curr == obj {
                    return Err(WorldStateError::RecursiveMove(*obj, *new_parent));
                }
                curr = self.parent_of(perms, &curr)?;
            }
        };

        let (objflags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);

        self.check_parent(perms, new_parent, &owner)?;
        self.perms(perms)?
            .check_object_allows(&owner, objflags, ObjFlag::Write.into())?;
        self.check_chparent_property_conflict(&owner, obj, new_parent)?;

        self.get_tx_mut().set_object_parent(obj, new_parent)
    }

    fn children_of(&self, _perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.children_of);
        self.get_tx().get_object_children(obj)
    }

    fn owned_objects(&self, _perms: &Obj, owner: &Obj) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.owned_objects);
        self.get_tx().get_owned_objects(owner)
    }

    fn descendants_of(
        &self,
        _perms: &Obj,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.descendants_of);
        self.get_tx().descendants(obj, include_self)
    }

    fn ancestors_of(
        &self,
        _perms: &Obj,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.ancestors_of);
        self.get_tx().ancestors(obj, include_self)
    }

    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.valid);
        self.get_tx().object_valid(obj)
    }

    fn name_of(&self, _perms: &Obj, obj: &Obj) -> Result<String, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.names_of);
        let name = self.get_tx().get_object_name(obj)?;

        Ok(name)
    }

    fn names_of(&self, perms: &Obj, obj: &Obj) -> Result<(String, Vec<String>), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.names_of);
        let name = self.get_tx().get_object_name(obj)?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(perms, obj, *ALIASES_SYM) {
            Ok(a) => match a.variant() {
                Variant::List(a) => a
                    .iter()
                    .map(|v| match v.variant() {
                        Variant::Str(s) => s.as_str().to_string(),
                        _ => "".to_string(),
                    })
                    .collect(),
                _ => {
                    vec![]
                }
            },
            Err(_) => {
                vec![]
            }
        };

        Ok((name, aliases))
    }

    fn increment_sequence(&self, seq: usize) -> i64 {
        self.get_tx().increment_sequence(seq)
    }

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.db_usage);
        self.get_tx().db_usage()
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.commit);
        self.tx.commit()
    }

    fn rollback(self: Box<Self>) -> Result<(), WorldStateError> {
        let _t = PerfTimerGuard::new(&WORLD_STATE_PERF.rollback);
        self.tx.rollback()
    }
}
