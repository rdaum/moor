// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// Affero General Public License as published by the Free Software Foundation,
// version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.

//! `WorldState` adapter backed by a single transaction.
//!
//! `DbWorldState` is the shared façade used by higher layers. It delegates
//! reads and writes to `WorldStateTransaction`, applies permission checks and
//! semantic rules, and records operation-level performance counters.

use std::collections::HashSet;
use std::sync::LazyLock;
use uuid::Uuid;

use crate::{
    api::auth::{AuthContext, AuthRule},
    api::gc::{GCError, GCInterface},
    engine::moor_db::WorldStateTransaction,
};
use moor_common::{
    model::{
        BuiltinProxyCacheBits, CommitResult, DispatchFlagsSource, HasUuid, ObjAttrs, ObjFlag,
        ObjSet, ObjectKind, ObjectQuery, ObjectRef, PropAttrs, PropDef, PropDefs, PropFlag,
        PropPerms, TaskPermissions, ValSet, VerbArgsSpec, VerbAttrs, VerbDef, VerbDefs,
        VerbDispatch, VerbDispatchResult, VerbFlag, VerbLookup, VerbProgramKey, WorldState,
        WorldStateError, WorldStatePerf, WorldStateTimerOp,
    },
    util::BitEnum,
};
use moor_var::{
    NOTHING, Obj, Symbol, Var, Variant,
    program::{ProgramType, opcode::BuiltinId},
    v_bool_int, v_list, v_obj,
};

static NAME_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("name"));
static LOCATION_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("location"));
static CONTENTS_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("contents"));
static OWNER_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("owner"));
static CHILDREN_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("children"));
static PARENT_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("parent"));
static PROGRAMMER_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("programmer"));
static WIZARD_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("wizard"));
static R_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("r"));
static W_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("w"));
static F_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("f"));
static ALIASES_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("aliases"));
static LAST_MOVE_SYM: LazyLock<Symbol> = LazyLock::new(|| Symbol::mk("last_move"));
static WORLD_STATE_PERF: LazyLock<WorldStatePerf> = LazyLock::new(WorldStatePerf::new);

thread_local! {
    static WORLD_STATE_PERF_TLS: &'static WorldStatePerf = &WORLD_STATE_PERF;
}

pub fn db_counters() -> &'static WorldStatePerf {
    WORLD_STATE_PERF_TLS.with(|c| *c)
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

    /// Extract the underlying transaction, consuming this DbWorldState.
    /// This allows reusing the same transaction with a different interface (e.g., LoaderInterface).
    pub fn into_transaction(self) -> WorldStateTransaction {
        self.tx
    }

    /// Create a DbWorldState from an existing transaction.
    /// This allows converting between WorldState and LoaderInterface using the same transaction.
    pub fn from_transaction(tx: WorldStateTransaction) -> Self {
        Self { tx }
    }
    fn auth_context<'a>(
        &self,
        permissions: &'a TaskPermissions,
    ) -> Result<AuthContext<'a>, WorldStateError> {
        let principal = permissions.principal();
        let flags = self.flags_of(&principal)?;
        Ok(AuthContext::with_grants(
            principal,
            flags,
            permissions.grants(),
        ))
    }

    #[inline]
    fn cached_auth_context<'a>(&self, permissions: &'a TaskPermissions) -> AuthContext<'a> {
        AuthContext::with_grants(
            permissions.principal(),
            permissions.principal_flags(),
            permissions.grants(),
        )
    }

    fn auth_context_for_principal(&self, who: &Obj) -> Result<AuthContext<'_>, WorldStateError> {
        let flags = self.flags_of(who)?;
        Ok(AuthContext::new(*who, flags))
    }

    fn update_property_internal(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError> {
        let auth = self.auth_context(permissions)?;

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

            if pname == *NAME_SYM {
                let Some(name) = value.as_string() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };

                // For player objects, only wizards can set the name.
                if flags.contains(ObjFlag::User) {
                    auth.require(AuthRule::property_wizard())?;
                }
                auth.require(AuthRule::object_rename(obj, &objowner))?;

                self.get_tx_mut().set_object_name(obj, name.to_string())?;
                return Ok(());
            }

            if pname == *OWNER_SYM {
                let Some(owner) = value.as_object() else {
                    return Err(WorldStateError::PropertyTypeMismatch);
                };
                auth.require(AuthRule::property_wizard())?;
                self.get_tx_mut().set_object_owner(obj, &owner)?;
                return Ok(());
            }

            auth.require(AuthRule::object_owner_or_wizard(&objowner))?;

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
            auth.require(AuthRule::property_wizard())?;

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

        if value.is_none() {
            return Err(WorldStateError::PropertyTypeMismatch);
        }

        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        auth.require(AuthRule::property_allows(
            obj,
            pname,
            &propperms,
            PropFlag::Write,
        ))?;
        self.get_tx_mut()
            .set_property(obj, pdef.uuid(), value.clone())?;
        Ok(())
    }

    fn do_update_verb(
        &mut self,
        obj: &Obj,
        permissions: &TaskPermissions,
        verbdef: &VerbDef,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let auth = self.auth_context(permissions)?;
        auth.require(AuthRule::verb_allows(
            obj,
            verbdef.uuid(),
            &verbdef.owner(),
            verbdef.flags(),
            VerbFlag::Write,
        ))?;

        // LambdaMOO/ToastStunt semantics: only wizards can transfer verb ownership.
        if let Some(new_owner) = verb_attrs.owner {
            auth.require(AuthRule::verb_owner_unchanged_or_wizard(
                &verbdef.owner(),
                &new_owner,
            ))?;
        }

        // If the verb code is being altered, require code-writing authority.
        if verb_attrs.program.is_some() {
            auth.require(AuthRule::verb_program(obj, verbdef.uuid()))?;
        }

        self.get_tx_mut()
            .update_verb(obj, verbdef.uuid(), verb_attrs)?;
        Ok(())
    }

    fn check_parent(
        &self,
        permissions: &TaskPermissions,
        parent: &Obj,
        owner: &Obj,
    ) -> Result<(), WorldStateError> {
        let (parentflags, parentowner) = (self.flags_of(parent)?, self.owner_of(parent)?);
        let auth = self.auth_context(permissions)?;
        if self.valid(parent)? {
            auth.require(AuthRule::object_allows(
                parent,
                &parentowner,
                parentflags,
                BitEnum::new_with(ObjFlag::Fertile),
            ))?;
        } else {
            if parent.ne(&NOTHING) {
                return Err(WorldStateError::ObjectPermissionDenied);
            }
            auth.require(AuthRule::object_owner_or_wizard(owner))?;
        }
        Ok(())
    }

    fn get_last_move_property(&self, obj: &Obj) -> Result<Var, WorldStateError> {
        self.get_tx().get_last_move(obj)
    }

    fn check_chparent_property_conflict(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError> {
        // If object or one of its descendants defines a property with the same name as one defined
        // either on new-parent or on one of its ancestors, then E_INVARG is raised.
        let obj_or_descendant_props = self
            .descendants_of(permissions, obj, true)?
            .iter()
            .map(|descendant| self.get_tx().get_properties(&descendant))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten();
        let new_parent_or_ancestors_property_names: HashSet<_> = self
            .ancestors_of(permissions, new_parent, true)?
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
    fn all_objects(&self) -> Result<ObjSet, WorldStateError> {
        self.get_tx().get_objects()
    }

    fn players(&self) -> Result<ObjSet, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::Players);
        self.get_tx().get_players()
    }

    fn owner_of(&self, obj: &Obj) -> Result<Obj, WorldStateError> {
        self.get_tx().get_object_owner(obj)
    }

    fn controls(&self, who: &Obj, what: &Obj) -> Result<bool, WorldStateError> {
        let owner = self.owner_of(what)?;
        Ok(self
            .auth_context_for_principal(who)?
            .allows(AuthRule::object_owner_or_wizard(&owner)))
    }

    fn flags_of(&self, obj: &Obj) -> Result<BitEnum<ObjFlag>, WorldStateError> {
        self.get_tx().get_object_flags(obj)
    }

    fn set_flags_of(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_flags: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::SetFlagsOf);
        let owner = self.owner_of(obj)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_owner_or_wizard(&owner))?;
        self.get_tx_mut().set_object_flags(obj, new_flags)
    }

    fn location_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<Obj, WorldStateError> {
        // MOO permits location query even if the object is unreadable!
        self.get_tx().get_object_location(obj)
    }

    fn object_bytes(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<usize, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ObjectBytes);
        self.auth_context(permissions)?
            .require(AuthRule::object_wizard())?;
        self.get_tx().get_object_size_bytes(obj)
    }

    fn create_object(
        &mut self,
        permissions: &TaskPermissions,
        parent: &Obj,
        owner: &Obj,
        flags: BitEnum<ObjFlag>,
        id_kind: ObjectKind,
    ) -> Result<Obj, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::CreateObject);
        let auth = self.auth_context(permissions)?;
        if !self.valid(parent)? && !parent.is_nothing() {
            return Err(WorldStateError::ObjectPermissionDenied);
        }
        auth.require(AuthRule::object_owner_or_wizard(owner))?;

        // Handle different ID kinds - validate specific IDs exist check
        match &id_kind {
            ObjectKind::Objid(obj_id) => {
                // If a specific ID is requested, check if it already exists
                if self.valid(obj_id)? {
                    return Err(WorldStateError::ObjectAlreadyExists(*obj_id));
                }
            }
            ObjectKind::NextObjid | ObjectKind::UuObjId | ObjectKind::Anonymous => {
                // No validation needed for auto-generated IDs
            }
        }

        self.check_parent(permissions, parent, owner)?;

        // TODO: ownership_quota support
        //    If the intended owner of the new object has a property named `ownership_quota' and the value of that property is an integer, then `create()' treats that value
        //    as a "quota".  If the quota is less than or equal to zero, then the quota is considered to be exhausted and `create()' raises `E_QUOTA' instead of creating an
        //    object.  Otherwise, the quota is decremented and stored back into the `ownership_quota' property as a part of the creation of the new object.
        let attrs = ObjAttrs::new(*owner, *parent, NOTHING, flags, "");
        self.get_tx_mut().create_object(id_kind, attrs)
    }

    fn check_recycle_object(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(), WorldStateError> {
        let owner = self.owner_of(obj)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_recycle(obj, &owner))
    }

    fn recycle_object(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::RecycleObject);
        self.check_recycle_object(permissions, obj)?;

        self.get_tx_mut().recycle_object(obj)
    }

    fn max_object(&self, _permissions: &TaskPermissions) -> Result<Obj, WorldStateError> {
        self.get_tx().get_max_object()
    }

    fn move_object(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_loc: &Obj,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::MoveObject);
        let owner = self.owner_of(obj)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_move(obj, &owner))?;

        // Get the old location before moving
        let old_loc = self.get_tx().get_object_location(obj)?;

        // Set the new location
        self.get_tx_mut().set_object_location(obj, new_loc)?;

        // Update last_move property with timestamp and source location
        self.get_tx_mut().set_last_move(obj, old_loc)?;

        Ok(())
    }

    fn contents_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<ObjSet, WorldStateError> {
        // MOO does not check authority for contents:
        // https://github.com/wrog/lambdamoo/blob/master/db_properties.c#L351
        self.get_tx().get_object_contents(obj)
    }

    fn verbs(&self, permissions: &TaskPermissions, obj: &Obj) -> Result<VerbDefs, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::Verbs);
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::object_allows(
                obj,
                &owner,
                flags,
                ObjFlag::Read.into(),
            ))?;

        self.get_tx().get_verbs(obj)
    }

    fn properties(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<PropDefs, WorldStateError> {
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::object_allows(
                obj,
                &owner,
                flags,
                ObjFlag::Read.into(),
            ))?;

        let properties = self.get_tx().get_properties(obj)?;
        Ok(properties)
    }

    #[allow(clippy::obfuscated_if_else)]
    fn retrieve_property(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Var, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::RetrieveProperty);
        if *obj == NOTHING || !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }

        // Special properties like name, location, and contents get treated specially.
        if pname == *NAME_SYM {
            return self.name_of(permissions, obj).map(Var::from);
        } else if pname == *LOCATION_SYM {
            return self.location_of(permissions, obj).map(Var::from);
        } else if pname == *CONTENTS_SYM {
            let contents: Vec<_> = self
                .contents_of(permissions, obj)?
                .iter()
                .map(v_obj)
                .collect();
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
        } else if pname == *LAST_MOVE_SYM {
            return self.get_last_move_property(obj);
        }

        let (_, value, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Read,
            ))?;
        Ok(value)
    }

    fn get_property_info(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(PropDef, PropPerms), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::GetPropertyInfo);
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Read,
            ))?;

        Ok((pdef.clone(), propperms))
    }

    fn snapshot_property(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<moor_common::model::PropertySnapshot, WorldStateError> {
        if !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }
        let definition = self.get_tx().find_property_by_uuid(obj, uuid)?;
        let effective_permissions = self.get_tx().retrieve_property_permissions(obj, uuid)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                definition.name(),
                &effective_permissions,
                PropFlag::Read,
            ))?;
        self.get_tx().snapshot_property(obj, definition)
    }

    fn set_property_info(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        attrs: PropAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::SetPropertyInfo);
        let auth = self.auth_context(permissions)?;
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        auth.require(AuthRule::property_allows(
            obj,
            pname,
            &propperms,
            PropFlag::Write,
        ))?;

        if let Some(flags) = attrs.flags
            && flags.contains(PropFlag::Clobber) != propperms.flags().contains(PropFlag::Clobber)
        {
            // Only wizards can change the property's conflict policy.
            auth.require(AuthRule::property_wizard())?;
        }

        // LambdaMOO/ToastStunt semantics: non-wizards may not transfer property ownership.
        if let Some(new_owner) = attrs.owner {
            auth.require(AuthRule::property_owner_unchanged_or_wizard(
                &propperms.owner(),
                &new_owner,
            ))?;
        }

        // TODO Also keep a close eye on 'clear' & MOO property-info perms:
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

    fn object_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError> {
        if !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::object_allows(
                obj,
                &owner,
                flags,
                ObjFlag::Read.into(),
            ))?;
        self.get_tx().object_metadata(obj)
    }

    fn get_object_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError> {
        if !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }
        let (flags, owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::object_allows(
                obj,
                &owner,
                flags,
                ObjFlag::Read.into(),
            ))?;
        self.get_tx().get_object_metadata(obj, key)
    }

    fn set_object_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError> {
        if !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }
        let owner = self.owner_of(obj)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_owner_or_wizard(&owner))?;
        self.get_tx_mut().set_object_metadata(obj, key, value)
    }

    fn clear_object_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        key: Symbol,
    ) -> Result<(), WorldStateError> {
        if !self.valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }
        let owner = self.owner_of(obj)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_owner_or_wizard(&owner))?;
        self.get_tx_mut().clear_object_metadata(obj, key)
    }

    fn property_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError> {
        let (pdef, _) = self.get_property_info(permissions, obj, pname)?;
        self.get_tx().property_metadata(obj, pdef.uuid())
    }

    fn get_property_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError> {
        let (pdef, _) = self.get_property_info(permissions, obj, pname)?;
        self.get_tx().get_property_metadata(obj, pdef.uuid(), key)
    }

    fn set_property_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError> {
        let (pdef, propperms) = self.get_property_info(permissions, obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Write,
            ))?;
        self.get_tx_mut()
            .set_property_metadata(obj, pdef.uuid(), key, value)
    }

    fn clear_property_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        key: Symbol,
    ) -> Result<(), WorldStateError> {
        let (pdef, propperms) = self.get_property_info(permissions, obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Write,
            ))?;
        self.get_tx_mut()
            .clear_property_metadata(obj, pdef.uuid(), key)
    }

    fn update_property(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
        value: &Var,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::UpdateProperty);
        self.update_property_internal(permissions, obj, pname, value)
    }

    fn is_property_clear(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<bool, WorldStateError> {
        let (_, _, propperms, clear) = self.get_tx().resolve_property(obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Read,
            ))?;
        Ok(clear)
    }

    fn clear_property(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ClearProperty);
        // This is just deleting the local *value* portion of the property.
        // First seek the property handle.
        let (pdef, _, propperms, _) = self.get_tx().resolve_property(obj, pname)?;
        self.auth_context(permissions)?
            .require(AuthRule::property_allows(
                obj,
                pname,
                &propperms,
                PropFlag::Write,
            ))?;
        if pdef.location() == *obj {
            return Err(WorldStateError::CannotClearPropertyOnDefiner(
                *obj,
                pname.to_string(),
            ));
        }
        self.get_tx_mut().clear_property(obj, pdef.uuid())?;
        Ok(())
    }

    fn define_property(
        &mut self,
        permissions: &TaskPermissions,
        definer: &Obj,
        location: &Obj,
        pname: Symbol,
        propowner: &Obj,
        prop_flags: BitEnum<PropFlag>,
        initial_value: Option<Var>,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::DefineProperty);

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
        {
            return Err(WorldStateError::PropertyPermissionDenied);
        }

        // Defining a property requires object-write-equivalent authority and authority over the
        // requested property owner.
        let (flags, objowner) = (self.flags_of(location)?, self.owner_of(location)?);
        let auth = self.auth_context(permissions)?;
        auth.require(AuthRule::property_define(location, &objowner, flags))?;
        auth.require(AuthRule::property_owner_or_wizard(propowner))?;
        if prop_flags.contains(PropFlag::Clobber) {
            auth.require(AuthRule::property_wizard())?;
        }

        if initial_value.as_ref().is_some_and(Var::is_none) {
            return Err(WorldStateError::PropertyTypeMismatch);
        }

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
        permissions: &TaskPermissions,
        obj: &Obj,
        pname: Symbol,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::DeleteProperty);
        let properties = self.get_tx().get_properties(obj)?;
        let pdef = properties
            .find_first_named(pname)
            .ok_or_else(|| WorldStateError::PropertyNotFound(*obj, pname.to_string()))?;
        let (objflags, objowner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::property_delete(obj, pname, &objowner, objflags))?;

        self.get_tx_mut().delete_property(obj, pdef.uuid())
    }

    fn add_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        names: Vec<Symbol>,
        owner: &Obj,
        flags: BitEnum<VerbFlag>,
        args: VerbArgsSpec,
        program: ProgramType,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::AddVerb);
        let (objflags, obj_owner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        let auth = self.auth_context(permissions)?;
        auth.require(AuthRule::verb_add(obj, &obj_owner, objflags))?;
        // LambdaMOO/ToastStunt semantics: non-wizards can only create verbs owned by themselves.
        auth.require(AuthRule::verb_owner_or_wizard(owner))?;

        self.get_tx_mut()
            .add_object_verb(obj, owner, &names, program, flags, args)?;
        Ok(())
    }

    fn remove_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        verb: Var,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::RemoveVerb);
        let (objflags, objowner) = (self.flags_of(obj)?, self.owner_of(obj)?);
        self.auth_context(permissions)?
            .require(AuthRule::object_allows(
                obj,
                &objowner,
                objflags,
                ObjFlag::Write.into(),
            ))?;

        let vh = match verb.variant() {
            Variant::Int(verb_index) => {
                if verb_index < 1 {
                    return Err(WorldStateError::VerbNotFound(*obj, verb_index.to_string()));
                }
                let verb_index = (verb_index as usize) - 1;
                self.get_tx().get_verb_by_index(obj, verb_index)?
            }
            _ => {
                let name = verb
                    .as_symbol()
                    .map_err(|_| WorldStateError::VerbNotFound(*obj, format!("{verb:?}")))?;
                self.get_tx().get_verb_by_name(obj, name)?
            }
        };

        self.get_tx_mut().delete_verb(obj, vh.uuid())?;
        Ok(())
    }

    fn update_verb(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vname: Symbol,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::UpdateVerb);
        let vh = self.get_tx().get_verb_by_name(obj, vname)?;
        self.do_update_verb(obj, permissions, &vh, verb_attrs)
    }

    fn update_verb_at_index(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vidx: usize,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::UpdateVerbAtIndex);
        let vh = self.get_tx().get_verb_by_index(obj, vidx)?;
        self.do_update_verb(obj, permissions, &vh, verb_attrs)
    }

    fn update_verb_with_id(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        verb_attrs: VerbAttrs,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::UpdateVerbWithId);
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.do_update_verb(obj, permissions, &vh, verb_attrs)
    }

    fn verb_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<Vec<(Symbol, Var)>, WorldStateError> {
        let _ = self.get_verb_by_id(permissions, obj, uuid)?;
        self.get_tx().verb_metadata(obj, uuid)
    }

    fn get_verb_metadata(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
    ) -> Result<Option<Var>, WorldStateError> {
        let _ = self.get_verb_by_id(permissions, obj, uuid)?;
        self.get_tx().get_verb_metadata(obj, uuid, key)
    }

    fn set_verb_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
        value: Var,
    ) -> Result<(), WorldStateError> {
        let vh = self
            .get_tx()
            .get_verbs(obj)?
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Write,
            ))?;
        self.get_tx_mut().set_verb_metadata(obj, uuid, key, value)
    }

    fn clear_verb_metadata(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
        key: Symbol,
    ) -> Result<(), WorldStateError> {
        let vh = self
            .get_tx()
            .get_verbs(obj)?
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Write,
            ))?;
        self.get_tx_mut().clear_verb_metadata(obj, uuid, key)
    }

    fn get_verb(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vname: Symbol,
    ) -> Result<VerbDef, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::GetVerb);
        if !self.get_tx().object_valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }

        let vh = self.get_tx().get_verb_by_name(obj, vname)?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Read,
            ))?;

        Ok(vh)
    }

    fn get_verb_at_index(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        vidx: usize,
    ) -> Result<VerbDef, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::GetVerbAtIndex);
        let vh = self.get_tx().get_verb_by_index(obj, vidx)?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Read,
            ))?;
        Ok(vh)
    }

    fn get_verb_by_id(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<VerbDef, WorldStateError> {
        let mut vh = None;
        for ancestor in self.get_tx().ancestors(obj, true)?.iter() {
            let verbs = self.get_tx().get_verbs(&ancestor)?;
            if let Some(verb) = verbs.find(&uuid) {
                vh = Some(verb);
                break;
            }
        }

        let vh = vh.ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        let auth = self.auth_context(permissions)?;
        if auth.allows(AuthRule::verb_allows(
            obj,
            vh.uuid(),
            &vh.owner(),
            vh.flags(),
            VerbFlag::Read,
        )) || auth.has_any_verb_grant(obj, vh.uuid())
        {
            return Ok(vh);
        }
        Err(WorldStateError::VerbPermissionDenied)
    }

    fn retrieve_verb(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::RetrieveVerb);
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Read,
            ))?;
        let binary = self.get_tx().get_verb_program(&vh.location(), vh.uuid())?;
        Ok((binary, vh))
    }

    fn retrieve_verb_for_execution(
        &self,
        permissions: &TaskPermissions,
        authorization_obj: &Obj,
        obj: &Obj,
        uuid: Uuid,
    ) -> Result<(ProgramType, VerbDef), WorldStateError> {
        let verbs = self.get_tx().get_verbs(obj)?;
        let vh = verbs
            .find(&uuid)
            .ok_or_else(|| WorldStateError::VerbNotFound(*obj, uuid.to_string()))?;
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                authorization_obj,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Exec,
            ))?;
        let binary = self.get_tx().get_verb_program(&vh.location(), vh.uuid())?;
        Ok((binary, vh))
    }

    fn lookup_verb(
        &self,
        permissions: &TaskPermissions,
        lookup: VerbLookup<'_>,
    ) -> Result<Option<VerbDef>, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::LookupVerb);
        if !self.valid(lookup.object)? {
            return Ok(None);
        }

        let vh = match self.get_tx().resolve_verb(
            lookup.object,
            lookup.verb_name,
            lookup.argspec,
            lookup.flagspec,
        ) {
            Ok(vh) => vh,
            Err(WorldStateError::VerbNotFound(_, _)) => return Ok(None),
            Err(e) => return Err(e),
        };
        self.auth_context(permissions)?
            .require(AuthRule::verb_allows(
                lookup.object,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Read,
            ))?;
        Ok(Some(vh))
    }

    fn dispatch_verb(
        &self,
        permissions: &TaskPermissions,
        dispatch: VerbDispatch<'_>,
    ) -> Result<Option<VerbDispatchResult>, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::DispatchVerb);
        if !self.valid(dispatch.lookup.object)? {
            return Ok(None);
        }

        let auth = self.cached_auth_context(permissions);
        let tx = self.get_tx();

        let vh = match tx.resolve_verb_handle(
            dispatch.lookup.object,
            dispatch.lookup.verb_name,
            dispatch.lookup.argspec,
            dispatch.lookup.flagspec,
        ) {
            Ok(vh) => vh,
            Err(WorldStateError::VerbNotFound(_, _)) => {
                if permissions.grants().is_empty() {
                    return Ok(None);
                }
                let candidate = match tx.resolve_verb_handle(
                    dispatch.lookup.object,
                    dispatch.lookup.verb_name,
                    dispatch.lookup.argspec,
                    None,
                ) {
                    Ok(vh) => vh,
                    Err(WorldStateError::VerbNotFound(_, _)) => return Ok(None),
                    Err(e) => return Err(e),
                };
                if auth.has_verb_call_grant(dispatch.lookup.object, candidate.uuid()) {
                    candidate
                } else {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        };

        // The x flag controls method calls. Command lookup uses the argument specification
        // and must also allow command-only verbs owned by another principal.
        if dispatch.lookup.argspec.is_none() {
            auth.require(AuthRule::verb_allows(
                dispatch.lookup.object,
                vh.uuid(),
                &vh.owner(),
                vh.flags(),
                VerbFlag::Exec,
            ))?;
        }
        let permissions_flags = match dispatch.flags_source {
            DispatchFlagsSource::Permissions => auth.principal_flags(),
            DispatchFlagsSource::VerbOwner => {
                if vh.owner() == auth.principal() {
                    auth.principal_flags()
                } else {
                    self.flags_of(&vh.owner()).unwrap_or_default()
                }
            }
        };
        Ok(Some(VerbDispatchResult {
            program_key: VerbProgramKey {
                verb_definer: vh.location(),
                verb_uuid: vh.uuid(),
            },
            verbdef: vh,
            permissions_flags,
        }))
    }

    fn builtin_proxy_cache_snapshot(&self) -> BuiltinProxyCacheBits {
        self.get_tx().builtin_proxy_cache_snapshot()
    }

    fn builtin_proxy_cache_guard_version(&self) -> i64 {
        self.get_tx().builtin_proxy_cache_guard_version()
    }

    fn mark_builtin_proxy_absent(&mut self, builtin: BuiltinId) {
        self.get_tx_mut().mark_builtin_proxy_absent(builtin);
    }

    fn parent_of(&self, _permissions: &TaskPermissions, obj: &Obj) -> Result<Obj, WorldStateError> {
        self.get_tx().get_object_parent(obj)
    }

    fn change_parent(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        new_parent: &Obj,
    ) -> Result<(), WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ChangeParent);
        {
            let mut curr = *new_parent;
            while !curr.is_nothing() {
                if &curr == obj {
                    return Err(WorldStateError::RecursiveMove(*obj, *new_parent));
                }
                curr = self.parent_of(permissions, &curr)?;
            }
        };

        let owner = self.owner_of(obj)?;

        self.check_parent(permissions, new_parent, &owner)?;
        self.auth_context(permissions)?
            .require(AuthRule::object_chparent(obj, &owner))?;
        self.check_chparent_property_conflict(permissions, obj, new_parent)?;

        self.get_tx_mut().set_object_parent(obj, new_parent)
    }

    fn children_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::ChildrenOf);
        self.get_tx().get_object_children(obj)
    }

    fn owned_objects(
        &self,
        _permissions: &TaskPermissions,
        owner: &Obj,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::OwnedObjects);
        self.get_tx().get_owned_objects(owner)
    }

    fn query_objects(&self, query: &ObjectQuery) -> Result<ObjSet, WorldStateError> {
        self.get_tx().query_objects(query)
    }

    fn descendants_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::DescendantsOf);
        self.get_tx().descendants(obj, include_self)
    }

    fn isa(&self, obj: &Obj, possible_ancestor: &Obj) -> Result<bool, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::Isa);
        self.get_tx().isa(obj, possible_ancestor)
    }

    fn ancestors_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
        include_self: bool,
    ) -> Result<ObjSet, WorldStateError> {
        let _t = db_counters()
            .timers_hot
            .start(WorldStateTimerOp::AncestorsOf);
        self.get_tx().ancestors(obj, include_self)
    }

    fn valid(&self, obj: &Obj) -> Result<bool, WorldStateError> {
        self.get_tx().object_valid(obj)
    }

    fn name_of(
        &self,
        _permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<String, WorldStateError> {
        let name = self.get_tx().get_object_name(obj)?;

        Ok(name)
    }

    fn names_of(
        &self,
        permissions: &TaskPermissions,
        obj: &Obj,
    ) -> Result<(String, Vec<String>), WorldStateError> {
        let name = self.get_tx().get_object_name(obj)?;

        // Then grab aliases property.
        let aliases = match self.retrieve_property(permissions, obj, *ALIASES_SYM) {
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

    fn renumber_object(
        &mut self,
        permissions: &TaskPermissions,
        obj: &Obj,
        target: Option<ObjectKind>,
    ) -> Result<Obj, WorldStateError> {
        use moor_common::model::ObjectRef;

        self.auth_context(permissions)?
            .require(AuthRule::object_wizard())?;

        // Check that source object exists
        if !self.get_tx().object_valid(obj)? {
            return Err(WorldStateError::ObjectNotFound(ObjectRef::Id(*obj)));
        }

        // Delegate to the transaction implementation
        self.get_tx_mut().renumber_object(obj, target)
    }

    fn persistence_metrics(&self) -> Vec<(&'static str, u64)> {
        self.get_tx().persistence_metrics()
    }

    fn db_usage(&self) -> Result<usize, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::DbUsage);
        self.get_tx().db_usage()
    }

    fn flush_caches(&mut self) {
        self.get_tx_mut().flush_caches();
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::Commit);
        self.tx.commit()
    }

    fn rollback(self: Box<Self>) -> Result<(), WorldStateError> {
        let _t = db_counters().timers_hot.start(WorldStateTimerOp::Rollback);
        self.tx.rollback()
    }

    fn as_loader_interface(
        self: Box<Self>,
    ) -> Result<Box<dyn moor_common::model::loader::LoaderInterface>, WorldStateError> {
        // Extract the transaction and re-wrap it - same transaction, different trait interface
        Ok(Box::new(DbWorldState { tx: self.tx }))
    }
}

impl GCInterface for DbWorldState {
    fn scan_anonymous_object_references(
        &mut self,
    ) -> Result<Vec<(Obj, HashSet<Obj>)>, WorldStateError> {
        self.get_tx_mut().scan_anonymous_object_references()
    }

    fn get_anonymous_objects(&self) -> Result<HashSet<Obj>, WorldStateError> {
        self.get_tx().get_anonymous_objects()
    }

    fn collect_unreachable_anonymous_objects(
        &mut self,
        unreachable_objects: &HashSet<Obj>,
    ) -> Result<usize, WorldStateError> {
        self.get_tx_mut()
            .collect_unreachable_anonymous_objects(unreachable_objects)
    }

    fn commit(self: Box<Self>) -> Result<CommitResult, GCError> {
        self.tx
            .commit()
            .map_err(|e| GCError::CommitFailed(e.to_string()))
    }

    fn rollback(self: Box<Self>) -> Result<(), GCError> {
        self.tx
            .rollback()
            .map_err(|e| GCError::CommitFailed(e.to_string()))
    }
}
