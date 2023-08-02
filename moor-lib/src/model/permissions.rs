use crate::model::objects::ObjFlag;

use crate::util::bitenum::BitEnum;
use crate::values::objid::{Objid, NOTHING};

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

    // Returns the permissions in use by the verb that *called* the currently-executing verb. If the
    // currently-executing verb was not called by another verb (i.e., it is the first verb called
    // in a command or server task), then #-1 is returned.
    // This is what is returned by bf caller_perms().
    caller_perms: Perms,

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
}

impl PermissionsContext {
    pub fn root_for(obj: Objid, flags: BitEnum<ObjFlag>) -> Self {
        let player_perms = Perms { obj, flags };
        Self {
            task_perms: player_perms.clone(),
            caller_perms: Perms {
                obj: NOTHING,
                flags: BitEnum::new(),
            },
            player_perms
        }
    }

    pub fn mk_child_perms(&self, new_task_perms: Perms) -> Self {
        Self {
            task_perms: new_task_perms.clone(),
            caller_perms: self.task_perms.clone(),
            player_perms: self.player_perms.clone(),
        }
    }

    pub fn has_flag(&self, flag: ObjFlag) -> bool {
        self.task_perms.flags.contains(flag)
    }

    pub fn task_perms(&self) -> &Perms {
        &self.task_perms
    }

    pub fn caller_perms(&self) -> &Perms {
        &self.caller_perms
    }

    pub fn set_task_perms(&mut self, obj: Objid, flags: BitEnum<ObjFlag>) {
        self.task_perms = Perms { obj, flags };
    }
}
