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

use crate::Obj;
use crate::model::objects::ObjFlag;
use crate::model::props::PropFlag;
use crate::model::verbs::VerbFlag;
use crate::model::{PropPerms, WorldStateError};
use crate::util::BitEnum;

/// Combination of who a set of permissions is for, and what permissions they have.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Perms {
    // "Who" the permissions are for
    pub who: Obj,
    // What flags apply for those permissions.
    pub flags: BitEnum<ObjFlag>,
}

impl Perms {
    #[must_use]
    pub fn new(obj: &Obj, flags: BitEnum<ObjFlag>) -> Self {
        Self {
            who: obj.clone(),
            flags,
        }
    }

    pub fn check_property_allows(
        &self,
        property_permissions: &PropPerms,
        allows: PropFlag,
    ) -> Result<(), WorldStateError> {
        if self.who == property_permissions.owner() {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        if !property_permissions.flags().contains(allows) {
            return Err(WorldStateError::PropertyPermissionDenied);
        }
        Ok(())
    }

    pub fn check_verb_allows(
        &self,
        verb_owner: &Obj,
        verb_flags: BitEnum<VerbFlag>,
        allows: VerbFlag,
    ) -> Result<(), WorldStateError> {
        if self.who.eq(verb_owner) {
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
        object_owner: &Obj,
        object_flags: BitEnum<ObjFlag>,
        allows: BitEnum<ObjFlag>,
    ) -> Result<(), WorldStateError> {
        if self.who.eq(object_owner) {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        if !object_flags.contains_all(allows) {
            return Err(WorldStateError::ObjectPermissionDenied);
        }
        Ok(())
    }

    pub fn check_obj_owner_perms(&self, object_owner: &Obj) -> Result<(), WorldStateError> {
        if self.who.eq(object_owner) {
            return Ok(());
        }
        if self.flags.contains(ObjFlag::Wizard) {
            return Ok(());
        }
        Err(WorldStateError::ObjectPermissionDenied)
    }

    pub fn check_wizard(&self) -> Result<(), WorldStateError> {
        if self.check_is_wizard()? {
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

    pub fn check_programmer(&self) -> Result<(), WorldStateError> {
        if self.check_is_programmer()? {
            return Ok(());
        }
        Err(WorldStateError::ObjectPermissionDenied)
    }

    pub fn check_is_programmer(&self) -> Result<bool, WorldStateError> {
        if self.flags.contains(ObjFlag::Programmer) {
            return Ok(true);
        }
        Ok(false)
    }
}
