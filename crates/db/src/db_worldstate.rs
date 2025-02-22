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

use bytes::Bytes;
use lazy_static::lazy_static;
use uuid::Uuid;

use moor_values::NOTHING;
use moor_values::Variant;
use moor_values::model::ObjSet;
use moor_values::model::Perms;
use moor_values::model::WorldState;
use moor_values::model::WorldStateError;
use moor_values::model::{ArgSpec, PrepSpec, VerbArgsSpec};
use moor_values::model::{BinaryType, VerbAttrs, VerbFlag};
use moor_values::model::{CommitResult, PropPerms, ValSet};
use moor_values::model::{HasUuid, ObjectRef};
use moor_values::model::{ObjAttrs, ObjFlag};
use moor_values::model::{PropAttrs, PropFlag};
use moor_values::model::{PropDef, PropDefs};
use moor_values::model::{VerbDef, VerbDefs};
use moor_values::util::BitEnum;
use moor_values::{Obj, v_bool_int};
use moor_values::{Symbol, v_list};
use moor_values::{Var, v_obj};

use crate::worldstate_transaction::WorldStateTransaction;

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
}

pub struct DbTxWorldState<TX: WorldStateTransaction> {
    pub tx: TX,
}

impl<TX> DbTxWorldState<TX>
where
    TX: WorldStateTransaction,
{
    pub(crate) fn get_tx(&self) -> &dyn WorldStateTransaction {
        &self.tx
    }

    pub(crate) fn get_tx_mut(&mut self) -> &mut dyn WorldStateTransaction {
        &mut self.tx
    }
    fn perms(&self, who: &Obj) -> Result<Perms, WorldStateError> {
        let flags = self.flags_of(who)?;
        Ok(Perms {
            who: who.clone(),
            flags,
        })
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
        if verb_attrs.binary.is_some()
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
}

impl<TX: WorldStateTransaction> WorldState for DbTxWorldState<TX> {
    fn players(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_players()
    }

    fn owner_of(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        self.get_tx().get_object_owner(obj)
    }

    fn controls(&self, who: &Obj, what: &Obj) -> Result<bool, WorldStateError> {
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
        self.get_tx().get_object_flags(obj)
    }

    fn set_flags_of(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        // Owner or wizard only.
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;
        self.get_tx_mut().set_object_flags(obj, new_flags)
    }

    fn location_of(&self, _perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError> {
        // MOO permits location query even if the object is unreadable!
        self.get_tx().get_object_location(obj)
    }

    fn object_bytes(&self, perms: &Obj, obj: &Obj) -> Result<usize, WorldStateError> {
        self.perms(perms)?.check_wizard()?;
        self.get_tx().get_object_size_bytes(obj)
    }

    fn create_object(
        &mut self,
        perms: &Obj,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
    ) -> Result<Obj, WorldStateError> {
        /*
           if ((valid(parent) ? !db_object_allows(parent, progr, FLAG_FERTILE)
                              : (parent != NOTHING)) || (owner != progr && !is_wizard(progr)))
               return make_error_pack(E_PERM);

           bool pe;
           if (valid(parent)) {
               pe = !db_object_allows(parent, progr, FLAG_FERTILE)
           } else {
               pe = (parent != NOTHING)) || (owner != progr && !is_wizard(progr)))
           }
        */

        self.check_parent(perms, parent, owner)?;

        // TODO: ownership_quota support
        //    If the intended owner of the new object has a property named `ownership_quota' and the value of that property is an integer, then `create()' treats that value
        //    as a "quota".  If the quota is less than or equal to zero, then the quota is considered to be exhausted and `create()' raises `E_QUOTA' instead of creating an
        //    object.  Otherwise, the quota is decremented and stored back into the `ownership_quota' property as a part of the creation of the new object.
        let attrs = ObjAttrs::new(owner.clone(), parent.clone(), NOTHING, flags, "");
        self.get_tx_mut().create_object(None, attrs)
    }

    fn recycle_object(&mut self, perms: &Obj, obj: &Obj) -> Result<(), WorldStateError> {
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;

        self.get_tx_mut().recycle_object(obj)
    }

    fn max_object(&self, _perms: &Obj) -> Result<Obj, WorldStateError> {
        self.get_tx().get_max_object()
    }

    fn move_object(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_loc: &Obj,
    ) -> Result<(), WorldStateError> {
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Write.into())?;

        self.get_tx_mut().set_object_location(obj, new_loc)
    }

    fn contents_of(&self, _perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        // MOO does not do any perms checks on contents, pretty sure:
        // https://github.com/wrog/lambdamoo/blob/master/db_properties.c#L351
        self.get_tx().get_object_contents(obj)
    }

    fn verbs(&self, perms: &Obj, obj: &Obj) -> Result<VerbDefs, WorldStateError> {
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Read.into())?;

        self.get_tx().get_verbs(obj)
    }

    fn properties(&self, perms: &Obj, obj: &Obj) -> Result<PropDefs, WorldStateError> {
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, flags, ObjFlag::Read.into())?;

        let properties = self.get_tx().get_properties(obj)?;
        Ok(properties)
    }

    fn retrieve_property(
        &self,
        perms: &Obj,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Var, WorldStateError> {
        if *obj == NOTHING || !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())));
        }

        // Special properties like name, location, and contents get treated specially.
        if pname == *NAME_SYM {
            return self.names_of(perms, obj).map(|(name, _)| Var::from(name));
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
                let Variant::Str(name) = value.variant() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                self.get_tx_mut()
                    .set_object_name(obj, name.as_string().clone())?;
                return Ok(());
            }

            if pname == *OWNER_SYM {
                let Variant::Obj(owner) = value.variant() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                self.get_tx_mut().set_object_owner(obj, owner)?;
                return Ok(());
            }

            if pname == *R_SYM {
                let Variant::Int(v) = value.variant() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if *v == 1 {
                    flags.set(ObjFlag::Read);
                } else {
                    flags.clear(ObjFlag::Read);
                }
                self.get_tx_mut().set_object_flags(obj, flags)?;
                return Ok(());
            }

            if pname == *W_SYM {
                let Variant::Int(v) = value.variant() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if *v == 1 {
                    flags.set(ObjFlag::Write);
                } else {
                    flags.clear(ObjFlag::Write);
                }
                self.get_tx_mut().set_object_flags(obj, flags)?;
                return Ok(());
            }

            if pname == *F_SYM {
                let Variant::Int(v) = value.variant() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                if *v == 1 {
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
        let properties = self.get_tx().get_properties(obj)?;
        let pdef = properties
            .find_first_named(pname)
            .ok_or(WorldStateError::PropertyNotFound(
                obj.clone(),
                pname.to_string(),
            ))?;
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
        binary: Vec<u8>,
        binary_type: BinaryType,
    ) -> Result<(), WorldStateError> {
        let (objflags, obj_owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&obj_owner, objflags, ObjFlag::Write.into())?;

        self.get_tx_mut()
            .add_object_verb(obj, owner, names, binary, binary_type, flags, args)?;
        Ok(())
    }

    fn remove_verb(&mut self, perms: &Obj, obj: &Obj, uuid: Uuid) -> Result<(), WorldStateError> {
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or(WorldStateError::VerbNotFound(obj.clone(), uuid.to_string()))?;
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
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or(WorldStateError::VerbNotFound(obj.clone(), uuid.to_string()))?;
        self.do_update_verb(obj, perms, &vh, verb_attrs)
    }

    fn get_verb(&self, perms: &Obj, obj: &Obj, vname: Symbol) -> Result<VerbDef, WorldStateError> {
        if !self.get_tx().object_valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(obj.clone())));
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
    ) -> Result<(Bytes, VerbDef), WorldStateError> {
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or(WorldStateError::VerbNotFound(obj.clone(), uuid.to_string()))?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;
        let binary = self.get_tx().get_verb_binary(&vh.location(), vh.uuid())?;
        Ok((binary, vh))
    }

    fn find_method_verb_on(
        &self,
        perms: &Obj,
        obj: &Obj,
        vname: Symbol,
    ) -> Result<(Bytes, VerbDef), WorldStateError> {
        let vh = self.get_tx().resolve_verb(obj, vname, None)?;
        self.perms(perms)?
            .check_verb_allows(&vh.owner(), vh.flags(), VerbFlag::Read)?;

        let binary = self.get_tx().get_verb_binary(&vh.location(), vh.uuid())?;
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
    ) -> Result<Option<(Bytes, VerbDef)>, WorldStateError> {
        if !self.valid(obj)? {
            return Ok(None);
        }

        let (objflags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.perms(perms)?
            .check_object_allows(&owner, objflags, ObjFlag::Read.into())?;

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

        let vh = self.get_tx().resolve_verb(obj, command_verb, Some(argspec));
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

        let binary = self.get_tx().get_verb_binary(&vh.location(), vh.uuid())?;
        Ok(Some((binary, vh)))
    }

    fn parent_of(&self, _perms: &Obj, obj: &Obj) -> Result<Obj, WorldStateError> {
        self.get_tx().get_object_parent(obj)
    }

    fn change_parent(
        &mut self,
        perms: &Obj,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError> {
        {
            let mut curr = new_parent.clone();
            while !curr.is_nothing() {
                if &curr == obj {
                    return Err(WorldStateError::RecursiveMove(
                        obj.clone(),
                        new_parent.clone(),
                    ));
                }
                curr = self.parent_of(perms, &curr)?.clone();
            }
        };

        let (objflags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);

        self.check_parent(perms, new_parent, &owner)?;
        self.perms(perms)?
            .check_object_allows(&owner, objflags, ObjFlag::Write.into())?;

        self.get_tx_mut().set_object_parent(obj, new_parent)
    }

    fn children_of(&self, _perms: &Obj, obj: &Obj) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_object_children(obj)
    }

    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError> {
        self.get_tx().object_valid(obj)
    }

    fn names_of(&self, perms: &Obj, obj: &Obj) -> Result<(String, Vec<String>), WorldStateError> {
        // Another thing that MOO allows lookup of without permissions.
        // First get name
        let name = self.get_tx().get_object_name(obj)?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(perms, obj, *ALIASES_SYM) {
            Ok(a) => match a.variant() {
                Variant::List(a) => a
                    .iter()
                    .map(|v| match v.variant() {
                        Variant::Str(s) => s.as_string().clone(),
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

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        self.get_tx().db_usage()
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        self.tx.commit()
    }

    fn rollback(self: Box<Self>) -> Result<(), WorldStateError> {
        self.tx.rollback()
    }
}
