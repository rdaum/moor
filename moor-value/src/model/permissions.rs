use crate::util::bitenum::BitEnum;
use crate::var::objid::Objid;

use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::model::verbs::VerbFlag;
use crate::model::WorldStateError;

/// Combination of who a set of permissions is for, and what permissions they have.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Perms {
    // "Who" the permissions are for
    pub who: Objid,
    // What flags apply for those permissions.
    pub flags: BitEnum<ObjFlag>,
}

impl Perms {
    pub fn new(obj: Objid, flags: BitEnum<ObjFlag>) -> Self {
        Self { who: obj, flags }
    }

    pub fn check_property_allows(
        &self,
        property_owner: Objid,
        property_flags: BitEnum<PropFlag>,
        allows: PropFlag,
    ) -> Result<(), WorldStateError> {
        if self.who == property_owner {
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
        if self.who == verb_owner {
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
        if self.who == object_owner {
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
        if self.who == object_owner {
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
