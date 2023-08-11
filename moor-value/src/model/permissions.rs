use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;

use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::model::verbs::VerbFlag;
use crate::model::WorldStateError;

/// Holder of all context relevant for permissions when passed through for WorldState calls.
/// WorldState implementations are responsible for performing permission checks for individual
/// state accesses, based on this.
/// Information used here is:
///     the active user ("player"):
///     the previous caller
/// Philosophically, all mutating, shared-state, secure operations should be done through
/// WorldState calls, and the PermissionsContext should apply in all cases.
/// Why not just have WorldState own a copy of the PermissionsContext?
///     Because in reality in the MOO world the permissions can and do fluctuate through out
///     call graph -- set_task_perms, caller_perms -- and mutating the PermissionsContext
///     in WorldState, and instead a copy should just hang out on the Activation record in the
///     stack.
/// How the permissions are *applied* however is the business of each worldstate.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PermissionsContext {
    // The permissions that are applicable in the current calling executing verb.
    // Usually the owner of the current verb.
    // But can be overridden by set_task_perms (wizard only)
    task_perms: Perms,

    // The original perms of the player. Used to derive caller_perms for the next call.
    // That is, self.caller_perms = parent_frame.task_perms
    player_perms: Perms,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Perms {
    // "Who" the permissions are for
    pub obj: Objid,
    // What flags apply for those permissions.
    pub flags: BitEnum<ObjFlag>,
}

impl Perms {
    pub fn new(obj: Objid, flags: BitEnum<ObjFlag>) -> Self {
        Self { obj, flags }
    }

    pub fn check_property_allows(
        &self,
        property_owner: Objid,
        property_flags: BitEnum<PropFlag>,
        allows: PropFlag,
    ) -> Result<(), WorldStateError> {
        if self.obj == property_owner {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        if !property_flags.contains(allows) {
            return Err(WorldStateError::PropertyPermissionDenied);
        }
        Ok(())
    }

    pub fn check_verb_allows(
        &self,
        verb_owner: Objid,
        verb_flags: BitEnum<VerbFlag>,
        allows: VerbFlag,
    ) -> Result<(), WorldStateError> {
        if self.obj == verb_owner {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        if !verb_flags.contains(allows) {
            return Err(WorldStateError::VerbPermissionDenied);
        }
        Ok(())
    }

    pub fn check_object_allows(
        &self,
        object_owner: Objid,
        object_flags: BitEnum<ObjFlag>,
        allows: ObjFlag,
    ) -> Result<(), WorldStateError> {
        if self.obj == object_owner {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        if !object_flags.contains(allows) {
            return Err(WorldStateError::ObjectPermissionDenied);
        }
        Ok(())
    }

    pub fn check_obj_owner_perms(&self, object_owner: Objid) -> Result<(), WorldStateError> {
        if self.obj == object_owner {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        Err(WorldStateError::ObjectPermissionDenied)
    }

    pub fn check_wizard(&self) -> Result<(), WorldStateError> {
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        Err(WorldStateError::ObjectPermissionDenied)
    }

    pub fn check_is_wizard(&self) -> Result<bool, WorldStateError> {
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(true);
        }
        Ok(false)
    }
}

impl PermissionsContext {
    pub fn root_for(obj: Objid, flags: BitEnum<ObjFlag>) -> Self {
        let player_perms = Perms { obj, flags };
        Self {
            task_perms: player_perms.clone(),
            player_perms,
        }
    }

    pub fn mk_child_perms(&self, new_task_perms: Perms) -> Self {
        Self {
            task_perms: new_task_perms.clone(),
            player_perms: self.player_perms.clone(),
        }
    }

    pub fn has_flag(&self, flag: ObjFlag) -> bool {
        self.task_perms.flags.contains(flag)
    }

    pub fn task_perms(&self) -> &Perms {
        &self.task_perms
    }

    pub fn set_task_perms(&mut self, obj: Objid, flags: BitEnum<ObjFlag>) {
        self.task_perms = Perms { obj, flags };
    }
}
